//! This crate provides a high level API for accessing and configuring a MiniDSP device.
//! To get started, start by instantiating the right transport. If the device is locally
//! connected via USB, use [`transport::hid::HidTransport`]. If using the `WI-DG` or connecting to
//! an instance of this program running the `server` component, see [`transport::net::NetTransport`].
//!
//! ```no_run
//! use minidsp::{MiniDSP, device::DEVICE_2X4HD, transport, Channel, Gain};
//! use anyhow::Result;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Find a locally connected minidsp using usb hid, with the default vendor and product id.
//!     let transport =  Arc::new(transport::hid::HidTransport::with_product_id(0x2752, 0x0011)?);
//!
//!     // Instantiate a 2x4HD handler for this device.
//!     let dsp = MiniDSP::new(transport, &DEVICE_2X4HD);
//!     
//!     let status = dsp.get_master_status().await?;
//!     println!("Master volume: {:.1}", status.volume.0);
//!
//!     // Activate a different configuration
//!     dsp.set_config(2).await?;
//!
//!     // Set the input gain for both input channels
//!     for i in 0..2 {
//!         dsp.input(i).set_gain(Gain(-10.)).await?;
//!     }
//!
//!     // Mute the last output channel
//!     dsp.output(3).set_mute(true).await?;
//!
//!     Ok(())
//! }
//!
//!
//! ```   

pub use crate::commands::Gain;
use crate::commands::{
    roundtrip, FromMemory, MasterStatus, ReadFloats, ReadMemory, SetConfig, SetMute, SetSource,
    SetVolume, UnaryCommand, WriteBiquad, WriteBiquadBypass, WriteFloat, WriteInt,
};
use async_trait::async_trait;
use commands::ReadHardwareId;

pub type Result<T, E = MiniDSPError> = core::result::Result<T, E>;

use std::{cell::Cell, convert::TryInto};

pub mod commands;
pub mod device;
pub mod discovery;
pub mod lease;
pub mod packet;
pub mod server;
pub mod source;
pub mod transport;

use crate::device::{Gate, PEQ};
use crate::transport::MiniDSPError;
pub use source::Source;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use transport::Transport;

/// High-level MiniDSP Control API
pub struct MiniDSP<'a> {
    pub transport: Arc<dyn Transport>,
    pub device: &'a device::Device,

    device_info: Mutex<Cell<Option<DeviceInfo>>>,
}

impl<'a> MiniDSP<'a> {
    pub fn new(transport: Arc<dyn Transport>, device: &'a device::Device) -> Self {
        MiniDSP {
            transport,
            device,
            device_info: Mutex::new(Cell::new(None)),
        }
    }
}

impl MiniDSP<'_> {
    async fn roundtrip<C>(&self, cmd: C) -> Result<C::Response, MiniDSPError>
    where
        C: UnaryCommand,
    {
        roundtrip(self.transport.as_ref(), cmd).await
    }

    /// Returns a `MasterStatus` object containing the current state
    pub async fn get_master_status(&self) -> Result<MasterStatus> {
        let device_info = self.get_device_info().await?;
        let memory = self.roundtrip(ReadMemory::new(0xffd8, 4)).await?;
        Ok(MasterStatus::from_memory(&device_info, &memory)
            .map_err(|_| MiniDSPError::MalformedResponse)?)
    }

    /// Gets the current input levels
    pub async fn get_input_levels(&self) -> Result<Vec<f32>> {
        let view = self.roundtrip(ReadFloats::new(0x0044, 2)).await?;
        Ok(view.data)
    }

    /// Gets the current output levels
    pub async fn get_output_levels(&self) -> Result<Vec<f32>> {
        let view = self.roundtrip(ReadFloats::new(0x004a, 4)).await?;
        Ok(view.data)
    }

    /// Sets the current master volume
    pub async fn set_master_volume(&self, value: Gain) -> Result<()> {
        self.roundtrip(SetVolume::new(value)).await
    }

    /// Sets the current master mute status
    pub async fn set_master_mute(&self, value: bool) -> Result<()> {
        self.roundtrip(SetMute::new(value)).await
    }

    /// Sets the current input source
    pub async fn set_source(&self, source: &str) -> Result<()> {
        use std::str::FromStr;
        let device_info = self.get_device_info().await?;
        let source: Source = Source::from_str(source).map_err(|_| MiniDSPError::InvalidSource)?;

        self.roundtrip(SetSource::new(source.to_id(&device_info)))
            .await
    }

    /// Sets the active configuration
    pub async fn set_config(&self, index: u8) -> Result<()> {
        self.roundtrip(SetConfig::new(index)).await
    }

    /// Gets an object wrapping an input channel
    pub fn input(&self, index: usize) -> Input {
        Input {
            dsp: &self,
            spec: &self.device.inputs[index],
        }
    }

    /// Gets an object wrapping an output channel
    pub fn output(&self, index: usize) -> Output {
        Output {
            dsp: &self,
            spec: &self.device.outputs[index],
        }
    }

    /// Gets the hardware id and dsp version, used internally to determine per-device configuration
    pub async fn get_device_info(&self) -> Result<DeviceInfo> {
        let self_device_info = self.device_info.lock().await;
        if let Some(info) = self_device_info.get() {
            return Ok(info);
        }

        let hardware_id = self.roundtrip(ReadHardwareId {}).await?;
        let view = self.roundtrip(ReadMemory::new(0xffa1, 1)).await?;
        let info = DeviceInfo {
            hw_id: hardware_id[3],
            dsp_version: view.read_u8(0xffa1),
        };
        self_device_info.set(Some(info));
        Ok(info)
    }
}

/// Hardware id and dsp version
#[derive(Clone, Copy, Debug)]
pub struct DeviceInfo {
    pub hw_id: u8,
    pub dsp_version: u8,
}

#[async_trait]
pub trait Channel {
    /// internal: Returns the address for this channel to include mute/gain functions
    #[doc(hidden)]
    fn _channel(&self) -> (&MiniDSP, &device::Gate, &device::PEQ);

    /// Sets the current mute setting
    async fn set_mute(&self, value: bool) -> Result<()> {
        let (dsp, gate, _) = self._channel();
        dsp.roundtrip(WriteInt::mute(gate.enable, !value)).await
    }

    /// Sets the current gain setting
    async fn set_gain(&self, value: Gain) -> Result<()> {
        let (dsp, gate, _) = self._channel();
        dsp.roundtrip(WriteFloat::new(gate.gain, value.0)).await
    }

    /// Get an object for configuring the parametric equalizer associated to this channel
    fn peq(&self, index: usize) -> BiquadFilter<'_> {
        let (dsp, _, peq) = self._channel();
        BiquadFilter::new(dsp, peq.at(index))
    }
}

/// Input channel control
pub struct Input<'a> {
    dsp: &'a MiniDSP<'a>,
    spec: &'a device::Input,
}

impl<'a> Input<'a> {
    /// Sets whether this input is routed to the given output
    pub async fn set_output_enable(&self, output_index: usize, value: bool) -> Result<()> {
        self.dsp
            .roundtrip(WriteInt::mute(
                self.spec.routing[output_index].enable,
                value,
            ))
            .await
    }

    /// Sets the routing matrix gain for this [input, output_index] pair
    pub async fn set_output_gain(&self, output_index: usize, gain: Gain) -> Result<()> {
        self.dsp
            .roundtrip(WriteFloat::new(
                self.spec.routing[output_index].gain,
                gain.0,
            ))
            .await
    }
}

impl Channel for Input<'_> {
    fn _channel(&self) -> (&MiniDSP, &Gate, &PEQ) {
        (self.dsp, &self.spec.gate, &self.spec.peq)
    }
}

/// Output channel control
pub struct Output<'a> {
    dsp: &'a MiniDSP<'a>,
    spec: &'a device::Output,
}

impl<'a> Output<'a> {
    /// Sets the output mute setting for this channel
    pub async fn set_invert(&self, value: bool) -> Result<()> {
        self.dsp
            .roundtrip(WriteInt::new(self.spec.invert_addr, value as u8))
            .await
    }

    /// Sets the output gain setting
    pub async fn set_delay(&self, value: Duration) -> Result<()> {
        self.dsp
            .roundtrip(WriteFloat::new(
                self.spec.gate.gain,
                value.as_secs_f32() / 1e3,
            ))
            .await
    }
}

impl Channel for Output<'_> {
    fn _channel(&self) -> (&MiniDSP, &Gate, &PEQ) {
        (self.dsp, &self.spec.gate, &self.spec.peq)
    }
}

/// Helper object for controlling an on-device biquad filter
pub struct BiquadFilter<'a> {
    dsp: &'a MiniDSP<'a>,
    addr: u16,
}

impl<'a> BiquadFilter<'a> {
    pub(crate) fn new(dsp: &'a MiniDSP<'a>, addr: u16) -> Self {
        BiquadFilter { dsp, addr }
    }

    /// Sets the biquad coefficient for this filter.
    /// The coefficients should be in the following order:
    /// [ b0, b1, b2, a1, a2 ]
    pub async fn set_coefficients(&self, coefficients: &[f32]) -> Result<()> {
        if coefficients.len() != 5 {
            panic!("biquad coefficients are always 5 floating point values")
        }

        self.dsp
            .roundtrip(WriteBiquad::new(
                self.addr,
                coefficients.try_into().unwrap(),
            ))
            .await
    }

    /// Sets whether this filter is bypassed
    pub async fn set_bypass(&self, bypass: bool) -> Result<()> {
        self.dsp
            .roundtrip(WriteBiquadBypass::new(self.addr, bypass))
            .await
    }
}
