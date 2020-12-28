extern crate hidapi;
pub use crate::commands::Gain;
use crate::commands::{
    roundtrip, FromMemory, MasterStatus, ReadMemory, SetMute, SetSource, SetVolume,
};
use anyhow::{anyhow, Result};

use std::convert::TryFrom;

pub mod commands;
pub mod lease;
pub mod packet;
pub mod transport;
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

pub struct MiniDSP {
    pub transport: Box<dyn Transport>,
}

impl MiniDSP {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        MiniDSP { transport }
    }
}

impl MiniDSP {
    pub async fn get_master_status(&self) -> Result<MasterStatus> {
        let memory = roundtrip(
            self.transport.as_ref(),
            ReadMemory {
                addr: 0xffd8,
                size: 4,
            },
        )
        .await?;

        Ok(MasterStatus::from_memory(&memory)?)
    }

    pub async fn set_master_volume(&self, value: Gain) -> Result<()> {
        Ok(roundtrip(self.transport.as_ref(), SetVolume::new(value)).await?)
    }

    pub async fn set_master_mute(&self, value: bool) -> Result<()> {
        Ok(roundtrip(self.transport.as_ref(), SetMute::new(value)).await?)
    }

    pub async fn set_source(&self, source: Source) -> Result<()> {
        Ok(roundtrip(self.transport.as_ref(), SetSource::new(source)).await?)
    }
}
