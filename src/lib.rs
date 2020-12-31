// extern crate hidapi;
pub use crate::commands::Gain;
use crate::commands::{
    roundtrip, FromMemory, MasterStatus, ReadFloats, ReadMemory, SetConfig, SetMute, SetSource,
    SetVolume, WriteBiquad, WriteBiquadBypass, WriteBool, WriteFloat,
};
use anyhow::{anyhow, Result};

use std::convert::{TryFrom, TryInto};

pub mod commands;
pub mod device;
pub mod discovery;
pub mod lease;
pub mod packet;
pub mod server;
pub mod transport;

use crate::transport::MiniDSPError;
use std::str::FromStr;
use std::sync::Arc;
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

/// High-level device struct issuing commands to a transport
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
    /// Returns a `MasterStatus` object containing the current state
    pub async fn get_master_status(&self) -> Result<MasterStatus> {
        let memory = roundtrip(self.transport.as_ref(), ReadMemory::new(0xffd8, 4)).await?;

        Ok(MasterStatus::from_memory(&memory)?)
    }

    /// Gets the current input levels
    pub async fn get_input_levels(&self) -> Result<Vec<f32>> {
        let view = roundtrip(self.transport.as_ref(), ReadFloats::new(0x0044, 2)).await?;
        Ok(view.data)
    }

    /// Gets the current output levels
    pub async fn get_output_levels(&self) -> Result<Vec<f32>> {
        let view = roundtrip(self.transport.as_ref(), ReadFloats::new(0x004a, 4)).await?;
        Ok(view.data)
    }

    /// Sets the current master volume
    pub async fn set_master_volume(&self, value: Gain) -> Result<()> {
        Ok(roundtrip(self.transport.as_ref(), SetVolume::new(value)).await?)
    }

    /// Sets the current master mute status
    pub async fn set_master_mute(&self, value: bool) -> Result<()> {
        Ok(roundtrip(self.transport.as_ref(), SetMute::new(value)).await?)
    }

    /// Sets the current input source
    pub async fn set_source(&self, source: Source) -> Result<()> {
        Ok(roundtrip(self.transport.as_ref(), SetSource::new(source)).await?)
    }

    /// Sets the active configuration
    pub async fn set_config(&self, index: u8) -> Result<()> {
        Ok(roundtrip(self.transport.as_ref(), SetConfig::new(index)).await?)
    }

    pub fn input(&self, index: u16) -> Input {
        let addr = 0x1a + index;
        Input {
            dsp: &self,
            index,
            addr,
        }
    }
}

pub struct Input<'a> {
    dsp: &'a MiniDSP<'a>,
    index: u16,
    addr: u16,
}

impl<'a> Input<'a> {
    /// Sets the input mute setting
    pub async fn set_mute(&self, value: bool) -> Result<()> {
        Ok(roundtrip(
            self.dsp.transport.as_ref(),
            // The underlying value controls whether the channel is enabled. If it is disabled,
            // it is considered muted since no signal can go through.
            WriteBool::new(self.index, !value),
        )
        .await?)
    }

    /// Sets the input gain setting
    pub async fn set_gain(&self, value: Gain) -> Result<()> {
        Ok(roundtrip(
            self.dsp.transport.as_ref(),
            WriteFloat::new(self.addr, value.0),
        )
        .await?)
    }

    /// Sets whether this input is routed to the given output
    pub async fn set_output_enable(&self, output_index: usize, value: bool) -> Result<()> {
        Ok(roundtrip(
            self.dsp.transport.as_ref(),
            WriteBool::new(self.spec().routing[output_index].enable, value),
        )
        .await?)
    }

    /// Sets the routing matrix gain for this [input, output_index] pair
    pub async fn set_output_gain(&self, output_index: usize, gain: Gain) -> Result<()> {
        Ok(roundtrip(
            self.dsp.transport.as_ref(),
            WriteFloat::new(self.spec().routing[output_index].gain, gain.0),
        )
        .await?)
    }

    pub async fn peq(&self, index: usize) -> BiquadFilter<'_> {
        BiquadFilter::new(self.dsp, self.spec().peq.at(index))
    }

    fn spec(&self) -> &'a device::Input {
        &self.dsp.device.inputs[self.index as usize]
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
}

impl<'a> BiquadFilter<'a> {
    pub async fn set_coefficients(&self, coefficients: &[f32]) -> Result<()> {
        if coefficients.len() != 5 {
            panic!("biquad coefficients are always 5 floating point values")
        }

        Ok(roundtrip(
            self.dsp.transport.as_ref(),
            WriteBiquad::new(self.addr, coefficients.try_into().unwrap()),
        )
        .await?)
    }

    pub async fn set_bypass(&self, bypass: bool) -> Result<()> {
        Ok(roundtrip(
            self.dsp.transport.as_ref(),
            WriteBiquadBypass::new(self.addr, bypass),
        )
        .await?)
    }
}
