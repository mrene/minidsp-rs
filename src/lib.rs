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
//!     let hid_api = transport::hid::initialize_api()?;
//!     // Find a locally connected minidsp using usb hid, with the default vendor and product id.
//!     let transport =  Arc::new(transport::hid::HidTransport::with_product_id(&hid_api, 0x2752, 0x0011)?);
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

use std::sync::Arc;
use std::{cell::Cell, convert::TryInto};

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::time::Duration;

use commands::{read_floats, read_memory};
pub use source::Source;
use transport::Transport;

pub use crate::commands::Gain;
use crate::commands::{roundtrip, Commands, FromMemory, MasterStatus, Responses, Value};
use crate::device::{Gate, PEQ};
use crate::transport::MiniDSPError;

pub type Result<T, E = MiniDSPError> = core::result::Result<T, E>;

pub mod biquad;
pub mod commands;
pub mod config;
pub mod device;
pub mod discovery;
pub mod packet;
pub mod rew;
pub mod server;
pub mod source;
pub mod transport;
pub mod utils;
pub use biquad::Biquad;

/// High-level MiniDSP Control API
pub struct MiniDSP<'a> {
    pub transport: Arc<Transport>,
    pub device: &'a device::Device,

    device_info: Mutex<Cell<Option<DeviceInfo>>>,
}

impl<'a> MiniDSP<'a> {
    pub fn new(transport: Arc<Transport>, device: &'a device::Device) -> Self {
        MiniDSP {
            transport,
            device,
            device_info: Mutex::new(Cell::new(None)),
        }
    }
}

impl MiniDSP<'_> {
    async fn roundtrip_expect<ExpectFn>(
        &self,
        cmd: commands::Commands,
        expect: ExpectFn,
    ) -> Result<commands::Responses, MiniDSPError>
    where
        ExpectFn: Fn(&Responses) -> bool,
    {
        roundtrip(self.transport.as_ref(), cmd, expect).await
    }

    async fn roundtrip(
        &self,
        cmd: commands::Commands,
    ) -> Result<commands::Responses, MiniDSPError> {
        roundtrip(self.transport.as_ref(), cmd, |r| r.is_ack()).await
    }

    async fn write_dsp<T: Into<Value>>(&self, addr: u16, value: T) -> Result<()> {
        self.roundtrip(Commands::Write {
            addr,
            value: value.into(),
        })
        .await?
        .into_ack()
    }

    /// Returns a `MasterStatus` object containing the current state
    pub async fn get_master_status(&self) -> Result<MasterStatus> {
        let device_info = self.get_device_info().await?;
        let memory = read_memory(self.transport.as_ref(), 0xffd8, 8).await?;
        Ok(
            MasterStatus::from_memory(&device_info, &memory).map_err(|e| {
                MiniDSPError::MalformedResponse(format!("Couldn't convert to MemoryView: {:?}", e))
            })?,
        )
    }

    /// Gets the current input levels
    pub async fn get_input_levels(&self) -> Result<Vec<f32>> {
        Ok(read_floats(self.transport.as_ref(), 0x0044, 2).await?.data)
    }

    /// Gets the current output levels
    pub async fn get_output_levels(&self) -> Result<Vec<f32>> {
        Ok(read_floats(self.transport.as_ref(), 0x004a, 4).await?.data)
    }

    /// Sets the current master volume
    pub async fn set_master_volume(&self, value: Gain) -> Result<()> {
        self.roundtrip(Commands::SetVolume { value })
            .await?
            .into_ack()
    }

    /// Sets the current master mute status
    pub async fn set_master_mute(&self, value: bool) -> Result<()> {
        self.roundtrip(Commands::SetMute { value })
            .await?
            .into_ack()
    }

    /// Sets the current input source
    pub async fn set_source(&self, source: &str) -> Result<()> {
        use std::str::FromStr;
        let device_info = self.get_device_info().await?;
        let source: Source = Source::from_str(source).map_err(|_| MiniDSPError::InvalidSource)?;

        self.roundtrip(Commands::SetSource {
            source: source.to_id(&device_info),
        })
        .await?
        .into_ack()
    }

    /// Sets the active configuration
    pub async fn set_config(&self, config: u8) -> Result<()> {
        self.roundtrip_expect(
            Commands::SetConfig {
                config,
                reset: true,
            },
            |resp| resp.is_config_changed(),
        )
        .await?
        .into_config_changed()
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

        let hw_id = self
            .roundtrip_expect(Commands::ReadHardwareId, |r| r.is_hardware_id())
            .await?
            .into_hardware_id()?;

        let view = read_memory(self.transport.as_ref(), 0xffa1, 1).await?;

        let info = DeviceInfo {
            hw_id,
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
        dsp.roundtrip(Commands::mute(gate.enable, value))
            .await?
            .into_ack()
    }

    /// Sets the current gain setting
    async fn set_gain(&self, value: Gain) -> Result<()> {
        let (dsp, gate, _) = self._channel();
        dsp.write_dsp(gate.gain, value.0).await
    }

    /// Get an object for configuring the parametric equalizer associated to this channel
    fn peq(&self, index: usize) -> BiquadFilter<'_> {
        let (dsp, _, peq) = self._channel();
        BiquadFilter::new(dsp, peq.at(index))
    }

    fn peqs_all(&self) -> Vec<BiquadFilter<'_>> {
        let (dsp, _, peq) = self._channel();
        peq.iter().map(move |x| BiquadFilter::new(dsp, x)).collect()
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
            .roundtrip(Commands::mute(
                self.spec.routing[output_index].enable,
                value,
            ))
            .await?
            .into_ack()
    }

    /// Sets the routing matrix gain for this [input, output_index] pair
    pub async fn set_output_gain(&self, output_index: usize, gain: Gain) -> Result<()> {
        self.dsp
            .write_dsp(self.spec.routing[output_index].gain, gain.0)
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
    /// Sets the output mute setting
    pub async fn set_invert(&self, value: bool) -> Result<()> {
        self.dsp
            .write_dsp(self.spec.invert_addr, value as u16)
            .await
    }

    /// Sets the output delay setting
    pub async fn set_delay(&self, value: Duration) -> Result<()> {
        // Each delay increment is 0.010 ms
        // let value = value / Duration::from_micros(10);
        let value = value.as_micros() / 10;
        if value > 80 {
            return Err(MiniDSPError::InternalError(anyhow!(
                "Delay should be within [0, 80], was {:?}",
                value
            )));
        }

        let value = value as u16;
        self.dsp.write_dsp(self.spec.delay_addr, value).await
    }

    /// Helper for setting crossover settings
    pub fn crossover(&'_ self) -> Crossover<'_> {
        Crossover::new(self.dsp, &self.spec.xover)
    }

    /// Helper for setting compressor settings
    pub fn compressor(&'_ self) -> Compressor<'_> {
        Compressor::new(self.dsp, &self.spec.compressor)
    }

    /// Helper for setting fir settings
    pub fn fir(&'_ self) -> Fir<'_> {
        Fir::new(self.dsp, &self.spec.fir)
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

    pub async fn clear(&self) -> Result<()> {
        self.set_coefficients(Biquad::default().to_array().as_ref())
            .await
    }

    /// Sets the biquad coefficient for this filter.
    /// The coefficients should be in the following order:
    /// [ b0, b1, b2, a1, a2 ]
    pub async fn set_coefficients(&self, coefficients: &[f32]) -> Result<()> {
        if coefficients.len() != 5 {
            panic!("biquad coefficients are always 5 floating point values")
        }

        self.dsp
            .roundtrip(Commands::WriteBiquad {
                addr: self.addr,
                data: coefficients.try_into().unwrap(),
            })
            .await?
            .into_ack()
    }

    /// Sets whether this filter is bypassed
    pub async fn set_bypass(&self, bypass: bool) -> Result<()> {
        self.dsp
            .roundtrip(Commands::WriteBiquadBypass {
                addr: self.addr,
                value: bypass,
            })
            .await?
            .into_ack()
    }
}

pub struct Crossover<'a> {
    dsp: &'a MiniDSP<'a>,
    spec: &'a device::Crossover,
}

impl<'a> Crossover<'a> {
    pub fn new(dsp: &'a MiniDSP<'a>, spec: &'a device::Crossover) -> Self {
        Crossover { dsp, spec }
    }

    pub async fn clear(&self, group: usize) -> Result<()> {
        for addr in self.spec.peqs[group].iter() {
            BiquadFilter::new(self.dsp, addr).clear().await?;
        }

        Ok(())
    }

    /// Set the biquad coefficients for a given index within a group
    /// There are usually two groups (0 and 1), each grouping 4 biquads
    pub async fn set_coefficients(
        &self,
        group: usize,
        index: usize,
        coefficients: &[f32],
    ) -> Result<()> {
        let addr = self.spec.peqs[group].at(index);
        let filter = BiquadFilter::new(self.dsp, addr);
        filter.set_coefficients(coefficients).await
    }

    /// Sets the bypass for a given crossover biquad group.
    /// There are usually two groups (0 and 1), each grouping 4 biquads
    pub async fn set_bypass(&self, group: usize, bypass: bool) -> Result<()> {
        let addr = self.spec.bypass[group];
        self.dsp
            .roundtrip(Commands::WriteBiquadBypass {
                addr,
                value: bypass,
            })
            .await?
            .into_ack()
    }

    pub fn num_groups(&self) -> usize {
        self.spec.peqs.len()
    }

    pub fn num_filter_per_group(&self) -> usize {
        self.spec.peqs[0].len
    }
}

pub struct Compressor<'a> {
    dsp: &'a MiniDSP<'a>,
    spec: &'a device::Compressor,
}

impl<'a> Compressor<'a> {
    pub fn new(dsp: &'a MiniDSP<'a>, spec: &'a device::Compressor) -> Self {
        Self { dsp, spec }
    }

    pub async fn set_bypass(&self, value: bool) -> Result<()> {
        let value = if value {
            commands::WriteInt::BYPASSED
        } else {
            commands::WriteInt::ENABLED
        };

        self.dsp.write_dsp(self.spec.bypass, value).await
    }

    pub async fn set_threshold(&self, value: f32) -> Result<()> {
        self.dsp.write_dsp(self.spec.threshold, value).await
    }

    pub async fn set_ratio(&self, value: f32) -> Result<()> {
        self.dsp.write_dsp(self.spec.ratio, value).await
    }

    pub async fn set_attack(&self, value: f32) -> Result<()> {
        self.dsp.write_dsp(self.spec.attack, value).await
    }

    pub async fn set_release(&self, value: f32) -> Result<()> {
        self.dsp.write_dsp(self.spec.release, value).await
    }

    pub async fn get_level(&self) -> Result<f32> {
        let view = self
            .dsp
            .roundtrip(Commands::ReadFloats {
                addr: self.spec.meter,
                len: 1,
            })
            .await?
            .into_float_view()?;

        Ok(view.get(self.spec.meter))
    }
}

pub struct Fir<'a> {
    dsp: &'a MiniDSP<'a>,
    spec: &'a device::Fir,
}

impl<'a> Fir<'a> {
    pub fn new(dsp: &'a MiniDSP<'a>, spec: &'a device::Fir) -> Self {
        Self { dsp, spec }
    }

    pub async fn set_bypass(&self, bypass: bool) -> Result<()> {
        let value = if bypass {
            commands::WriteInt::BYPASSED
        } else {
            commands::WriteInt::ENABLED
        };

        self.dsp.write_dsp(self.spec.bypass, value).await
    }

    pub async fn clear(&self) -> Result<()> {
        self.set_coefficients([0.0].repeat(16).as_ref()).await
    }

    /// Loads all coefficients into the filter, automatically setting the number of active taps
    pub async fn set_coefficients(&self, coefficients: &[f32]) -> Result<()> {
        // The device will change the master mute status while loading the filter
        let master_status = self.dsp.get_master_status().await?;

        // Set the number of active coefficients
        self.dsp
            .write_dsp(self.spec.num_coefficients, coefficients.len() as u16)
            .await?;

        // Get the max number of usable coefficients
        let max_coeff = self
            .dsp
            .roundtrip_expect(
                Commands::FirLoadStart {
                    index: self.spec.index,
                },
                |r| r.is_fir_size(),
            )
            .await?
            .into_fir_size()?;

        if coefficients.len() > max_coeff as usize {
            return Err(MiniDSPError::TooManyCoefficients);
        }

        // Load coefficients by chunk of 14 floats
        for block in coefficients.chunks(14) {
            self.dsp
                .roundtrip(Commands::FirLoadData {
                    index: self.spec.index,
                    data: Vec::from(block),
                })
                .await?
                .into_ack()?;
        }

        // Send load end
        self.dsp.roundtrip(Commands::FirLoadEnd).await?.into_ack()?;

        // Set the master mute status back
        self.dsp.set_master_mute(master_status.mute).await?;

        Ok(())
    }
}
