//! This crate provides a high level API for accessing and configuring a MiniDSP device.
//! To get started, start by instantiating the right transport. If the device is locally
//! connected via USB, use [`transport::hid::find_minidsp`]. If using the `WI-DG` or connecting to
//! an instance of this program running the `server` component, see [`transport::net::NetTransport::new`].
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
use anyhow::anyhow;
use async_trait::async_trait;

pub type Result<T, E = MiniDSPError> = core::result::Result<T, E>;

use std::convert::{TryFrom, TryInto};

pub mod commands;
pub mod device;
pub mod discovery;
pub mod lease;
pub mod packet;
pub mod server;
pub mod transport;

use crate::device::{Gate, PEQ};
use crate::transport::MiniDSPError;
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::Duration;
use transport::Transport;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Source {
    Analog,
    Toslink,
    Usb,
}

impl TryFrom<u8> for Source {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Source::Analog),
            1 => Ok(Source::Toslink),
            2 => Ok(Source::Usb),
            _ => Err(anyhow!("Invalid source value")),
        }
    }
}

impl Into<u8> for Source {
    fn into(self) -> u8 {
        match self {
            Source::Analog => 0,
            Source::Toslink => 1,
            Source::Usb => 2,
        }
    }
}

impl FromStr for Source {
    type Err = MiniDSPError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Source::*;

        match s.to_lowercase().as_str() {
            "analog" => Ok(Analog),
            "toslink" => Ok(Toslink),
            "usb" => Ok(Usb),
            _ => Err(MiniDSPError::InvalidSource),
        }
    }
}

/// High-level MiniDSP Control API
pub struct MiniDSP<'a> {
    pub transport: Arc<dyn Transport>,
    pub device: &'a device::Device,
}

impl<'a> MiniDSP<'a> {
    pub fn new(transport: Arc<dyn Transport>, device: &'a device::Device) -> Self {
        MiniDSP { transport, device }
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
        let memory = self.roundtrip(ReadMemory::new(0xffd8, 4)).await?;
        Ok(MasterStatus::from_memory(&memory).map_err(|_| MiniDSPError::MalformedResponse)?)
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
    pub async fn set_source(&self, source: Source) -> Result<()> {
        self.roundtrip(SetSource::new(source)).await
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

    pub fn output(&self, index: usize) -> Output {
        Output {
            dsp: &self,
            spec: &self.device.outputs[index],
        }
    }
}

#[async_trait]
pub trait Channel {
    /// [internal] Returns the address for this channel to include mute/gain functions
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
    pub fn new(dsp: &'a MiniDSP<'a>, addr: u16) -> Self {
        BiquadFilter { dsp, addr }
    }

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

    pub async fn set_bypass(&self, bypass: bool) -> Result<()> {
        self.dsp
            .roundtrip(WriteBiquadBypass::new(self.addr, bypass))
            .await
    }
}
