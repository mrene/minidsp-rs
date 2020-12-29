// extern crate hidapi;
pub use crate::commands::Gain;
use crate::commands::{
    roundtrip, FromMemory, MasterStatus, ReadFloats, ReadMemory, SetMute, SetSource, SetVolume,
};
use anyhow::{anyhow, Result};

use std::convert::TryFrom;

pub mod commands;
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
pub struct MiniDSP {
    pub transport: Arc<dyn Transport>,
}

impl MiniDSP {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        MiniDSP { transport }
    }
}

impl MiniDSP {
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
}

// TODO: Device spec
// Map available inputs per hwid+dspversion
// Number of in/outs and their addresses in float space
