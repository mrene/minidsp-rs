//! This crate provides a high level API for accessing and configuring a MiniDSP device.
//! To get started, start by instantiating the right transport. If the device is locally
//! connected via USB, use [`transport::hid::HidTransport`]. If using the `WI-DG` or connecting to
//! an instance of this program running the `server` component, see [`transport::net::NetTransport`].
//!
//! ```no_run
//! use anyhow::Result;
//! use futures::StreamExt;
//! use minidsp::{
//!     transport::{hid, Multiplexer},
//!     Builder, Channel, Gain, MiniDSP,
//! };
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Get a list of local devices
//!     let mut builder = Builder::new();
//!     builder.with_default_usb().unwrap();
//!
//!     let mut devices: Vec<_> = builder
//!         // Probe each candidate device for its hardware id and serial number
//!         .probe()
//!         // Filter the list to keep the working devices
//!         .filter_map(|x| async move { x.ok() })
//!         .collect()
//!         .await;
//!
//!     // Use the first device for further commands
//!     let dsp = devices.first().unwrap().to_minidsp().unwrap();
//!
//!     let status = dsp.get_master_status().await?;
//!     println!("Master volume: {:.1}", status.volume.unwrap().0);
//!
//!     // Activate a different configuration
//!     dsp.set_config(2).await?;
//!
//!     // Set the input gain for both input channels
//!     for i in 0..2 {
//!         dsp.input(i)?.set_gain(Gain(-10.)).await?;
//!     }
//!
//!     // Mute the last output channel
//!     dsp.output(3)?.set_mute(true).await?;
//!
//!     Ok(())
//! }
//! ```   

// Silence clippy warning inside JsonSchema derived code
#![allow(clippy::field_reassign_with_default)]
// Silence naming until we move to 0.1.0
#![allow(clippy::upper_case_acronyms)]

use std::convert::TryInto;

use anyhow::anyhow;
use async_trait::async_trait;
use client::Client;
use futures::{Stream, StreamExt};
pub use minidsp_protocol::{Commands, DeviceInfo, FromMemory, MasterStatus, Source};
use tokio::time::Duration;
pub use transport::MiniDSPError;
use utils::ErrInto;

pub use crate::commands::Gain;

pub type Result<T, E = MiniDSPError> = core::result::Result<T, E>;

pub mod biquad;
pub use minidsp_protocol::{commands, device, device::Gate, packet};
pub mod tcp_server;
pub use minidsp_protocol::source;
pub mod transport;
pub mod utils;
pub use biquad::Biquad;
pub mod builder;
pub use builder::Builder;
pub mod client;
pub mod formats;
pub mod logging;
pub mod model;

/// High-level MiniDSP Control API
#[derive(Clone)]
pub struct MiniDSP<'a> {
    pub client: Client,
    pub device: &'a device::Device,

    device_info: DeviceInfo,
}

impl<'a> MiniDSP<'a> {
    pub fn from_client(
        client: Client,
        device: &'a device::Device,
        device_info: DeviceInfo,
    ) -> Self {
        MiniDSP {
            client,
            device,
            device_info,
        }
    }
}

impl MiniDSP<'_> {
    /// Returns a `MasterStatus` object containing the current state
    pub async fn get_master_status(&self) -> Result<MasterStatus> {
        let device_info = self.device_info;
        let memory = self.client.read_memory(0xffd8, 9).await?;

        Ok(
            MasterStatus::from_memory(&device_info, &memory).map_err(|e| {
                MiniDSPError::MalformedResponse(format!("Couldn't convert to MemoryView: {:?}", e))
            })?,
        )
    }

    pub async fn subscribe_master_status(
        &self,
    ) -> Result<impl Stream<Item = MasterStatus> + 'static, MiniDSPError> {
        let device_info = self.device_info;
        let stream = self
            .client
            .subscribe()
            .await?
            .filter_map(move |item| async move {
                if let commands::Responses::MemoryData(memory) = item.ok()? {
                    let status = MasterStatus::from_memory(&device_info, &memory).ok()?;
                    if !status.eq(&MasterStatus::default()) {
                        return Some(status);
                    }
                }
                None
            });
        Ok(Box::pin(stream))
    }

    // Gets the current input and output level using a single command
    pub async fn get_input_output_levels(&self) -> Result<(Vec<f32>, Vec<f32>)> {
        let mut levels = self
            .client
            .read_floats_multi(
                self.device
                    .inputs
                    .iter()
                    .map(|idx| idx.meter)
                    .chain(self.device.outputs.iter().map(|idx| idx.meter)),
            )
            .await?;

        let outputs = Vec::from(&levels[self.device.inputs.len()..levels.len()]);
        levels.truncate(self.device.inputs.len());

        Ok((levels, outputs))
    }

    /// Gets the current input levels
    pub async fn get_input_levels(&self) -> Result<Vec<f32>> {
        self.client
            .read_floats_multi(self.device.inputs.iter().map(|idx| idx.meter))
            .await
    }

    /// Gets the current output levels
    pub async fn get_output_levels(&self) -> Result<Vec<f32>> {
        self.client
            .read_floats_multi(self.device.outputs.iter().map(|idx| idx.meter))
            .await
    }

    /// Sets the current master volume
    pub async fn set_master_volume(&self, value: Gain) -> Result<()> {
        self.client
            .roundtrip(Commands::SetVolume { value })
            .await?
            .into_ack()
            .err_into()
    }

    /// Sets the current master mute status
    pub async fn set_master_mute(&self, value: bool) -> Result<()> {
        self.client
            .roundtrip(Commands::SetMute { value })
            .await?
            .into_ack()
            .err_into()
    }

    /// Sets the current input source
    pub async fn set_source(&self, source: Source) -> Result<()> {
        let device_info = self.get_device_info().await?;

        self.client
            .roundtrip(Commands::SetSource {
                source: source.to_id(&device_info),
            })
            .await?
            .into_ack()
            .err_into()
    }

    /// Sets the active configuration
    pub async fn set_config(&self, config: u8) -> Result<()> {
        self.client
            .roundtrip(Commands::SetConfig {
                config,
                reset: true,
            })
            .await?
            .into_config_changed()
            .err_into()
    }

    /// Enables or disables Dirac Live
    pub async fn set_dirac(&self, enabled: bool) -> Result<()> {
        self.client
            .roundtrip(Commands::DiracBypass {
                value: if enabled { 0 } else { 1 },
            })
            .await?
            .into_ack()
            .err_into()
    }

    /// Gets an object wrapping an input channel
    pub fn input(&self, index: usize) -> Result<Input> {
        if index >= self.device.inputs.len() {
            Err(MiniDSPError::OutOfRange)
        } else {
            Ok(Input {
                dsp: &self,
                spec: &self.device.inputs[index],
            })
        }
    }

    /// Gets an object wrapping an output channel
    pub fn output(&self, index: usize) -> Result<Output> {
        if index >= self.device.outputs.len() {
            Err(MiniDSPError::OutOfRange)
        } else {
            Ok(Output {
                dsp: &self,
                spec: &self.device.outputs[index],
            })
        }
    }

    /// Gets the hardware id and dsp version, used internally to determine per-device configuration
    pub async fn get_device_info(&self) -> Result<DeviceInfo> {
        Ok(self.device_info)
    }
}

#[async_trait]
pub trait Channel {
    /// internal: Returns the address for this channel to include mute/gain functions
    #[doc(hidden)]
    fn _channel(&self) -> (&MiniDSP, &device::Gate, &'static [u16]);

    /// Sets the current mute setting
    async fn set_mute(&self, value: bool) -> Result<()> {
        let (dsp, gate, _) = self._channel();
        dsp.client
            .roundtrip(Commands::mute(gate.enable, value))
            .await?
            .into_ack()
            .err_into()
    }

    /// Sets the current gain setting
    async fn set_gain(&self, value: Gain) -> Result<()> {
        let (dsp, gate, _) = self._channel();
        dsp.client.write_dsp(gate.gain, value.0).await
    }

    /// Get an object for configuring the parametric equalizer associated to this channel
    fn peq(&self, index: usize) -> Result<BiquadFilter<'_>> {
        let (dsp, _, peq) = self._channel();
        if index >= peq.len() {
            Err(MiniDSPError::OutOfRange)
        } else {
            Ok(BiquadFilter::new(dsp, peq[index]))
        }
    }

    fn peqs_all(&self) -> Vec<BiquadFilter<'_>> {
        let (dsp, _, peq) = self._channel();
        peq.iter()
            .map(move |x| BiquadFilter::new(dsp, *x))
            .collect()
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
            .client
            .roundtrip(Commands::mute(
                self.spec.routing[output_index].enable,
                value,
            ))
            .await?
            .into_ack()
            .err_into()
    }

    /// Sets the routing matrix gain for this [input, output_index] pair
    pub async fn set_output_gain(&self, output_index: usize, gain: Gain) -> Result<()> {
        self.dsp
            .client
            .write_dsp(self.spec.routing[output_index].gain, gain.0)
            .await
            .err_into()
    }
}

impl Channel for Input<'_> {
    fn _channel(&self) -> (&MiniDSP, &Gate, &'static [u16]) {
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
            .client
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
        self.dsp.client.write_dsp(self.spec.delay_addr, value).await
    }

    /// Helper for setting crossover settings
    pub fn crossover(&'_ self) -> Option<Crossover<'_>> {
        Some(Crossover::new(self.dsp, self.spec.xover.as_ref()?))
    }

    /// Helper for setting compressor settings
    pub fn compressor(&'_ self) -> Option<Compressor<'_>> {
        Some(Compressor::new(self.dsp, self.spec.compressor.as_ref()?))
    }

    /// Helper for setting fir settings
    pub fn fir(&'_ self) -> Option<Fir<'_>> {
        Some(Fir::new(self.dsp, self.spec.fir.as_ref()?))
    }
}

impl Channel for Output<'_> {
    fn _channel(&self) -> (&MiniDSP, &Gate, &'static [u16]) {
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
            .client
            .roundtrip(Commands::WriteBiquad {
                addr: self.addr,
                data: coefficients.try_into().unwrap(),
            })
            .await?
            .into_ack()
            .err_into()
    }

    /// Sets whether this filter is bypassed
    pub async fn set_bypass(&self, bypass: bool) -> Result<()> {
        self.dsp
            .client
            .roundtrip(Commands::WriteBiquadBypass {
                addr: self.addr,
                value: bypass,
            })
            .await?
            .into_ack()
            .err_into()
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
        let start = self.spec.peqs[group];
        for addr in (start..(start + 5 * 4)).step_by(5) {
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
        if group >= self.num_groups() || index >= self.num_filter_per_group() {
            return Err(MiniDSPError::OutOfRange);
        }

        let addr = self.spec.peqs[group] + (index as u16) * 5;
        let filter = BiquadFilter::new(self.dsp, addr);
        filter.set_coefficients(coefficients).await
    }

    /// Sets the bypass for a given crossover biquad group.
    /// There are usually two groups (0 and 1), each grouping 4 biquads
    pub async fn set_bypass(&self, group: usize, bypass: bool) -> Result<()> {
        if group >= self.num_groups() {
            return Err(MiniDSPError::OutOfRange);
        }

        let addr = self.spec.peqs[group];
        self.dsp
            .client
            .roundtrip(Commands::WriteBiquadBypass {
                addr,
                value: bypass,
            })
            .await?
            .into_ack()
            .err_into()
    }

    pub fn num_groups(&self) -> usize {
        self.spec.peqs.len()
    }

    pub fn num_filter_per_group(&self) -> usize {
        4
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

        self.dsp.client.write_dsp(self.spec.bypass, value).await
    }

    pub async fn set_threshold(&self, value: f32) -> Result<()> {
        self.dsp.client.write_dsp(self.spec.threshold, value).await
    }

    pub async fn set_ratio(&self, value: f32) -> Result<()> {
        self.dsp.client.write_dsp(self.spec.ratio, value).await
    }

    pub async fn set_attack(&self, value: f32) -> Result<()> {
        self.dsp.client.write_dsp(self.spec.attack, value).await
    }

    pub async fn set_release(&self, value: f32) -> Result<()> {
        self.dsp.client.write_dsp(self.spec.release, value).await
    }

    pub async fn get_level(&self) -> Result<f32> {
        let meter = self.spec.meter.ok_or(MiniDSPError::NoSuchPeripheral)?;
        let view = self
            .dsp
            .client
            .roundtrip(Commands::ReadFloats {
                addr: meter,
                len: 1,
            })
            .await?
            .into_float_view()?;

        Ok(view.get(meter))
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

        self.dsp.client.write_dsp(self.spec.bypass, value).await
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
            .client
            .write_dsp(self.spec.num_coefficients, coefficients.len() as u16)
            .await?;

        // Get the max number of usable coefficients
        let max_coeff = self
            .dsp
            .client
            .roundtrip(Commands::FirLoadStart {
                index: self.spec.index,
            })
            .await?
            .into_fir_size()?;

        if coefficients.len() > max_coeff as usize {
            return Err(MiniDSPError::TooManyCoefficients);
        }

        // Load coefficients by chunk of 14 floats
        for block in coefficients.chunks(14) {
            self.dsp
                .client
                .roundtrip(Commands::FirLoadData {
                    index: self.spec.index,
                    data: Vec::from(block),
                })
                .await?
                .into_ack()?;
        }

        // Send load end
        self.dsp
            .client
            .roundtrip(Commands::FirLoadEnd)
            .await?
            .into_ack()?;

        // Set the master mute status back
        self.dsp
            .set_master_mute(master_status.mute.unwrap())
            .await?;

        Ok(())
    }
}
