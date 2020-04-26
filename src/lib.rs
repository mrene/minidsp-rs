extern crate failure;
extern crate hidapi;

use std::convert::TryFrom;

use hidapi::{HidApi, HidError};

use crate::commands::{FromMemory, MasterStatus, ReadMemory, SetMute, SetSource, SetVolume};

pub mod commands;
pub mod lease;
pub mod transport;

pub use crate::commands::Gain;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Source {
    Analog,
    Toslink,
    Usb,
}

impl TryFrom<u8> for Source {
    type Error = failure::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Source::Analog),
            1 => Ok(Source::Toslink),
            2 => Ok(Source::Usb),
            _ => Err(failure::format_err!("Invalid source value")),
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
    pub transport: Box<dyn transport::Transport>,
}

impl MiniDSP {
    pub fn new(transport: Box<dyn transport::Transport>) -> Self {
        MiniDSP { transport }
    }
}

impl MiniDSP {
    pub fn get_master_status(&mut self) -> Result<MasterStatus, failure::Error> {
        let memory = ReadMemory {
            addr: 0xffd8,
            size: 4,
        }
        .execute(self.transport.as_mut())?;
        let master_status = MasterStatus::from_memory(&memory)?;
        Ok(master_status)
    }

    pub fn set_master_volume(&mut self, value: Gain) -> Result<(), failure::Error> {
        SetVolume::new(value).execute(self.transport.as_mut())
    }

    pub fn set_master_mute(&mut self, value: bool) -> Result<(), failure::Error> {
        SetMute::new(value).execute(self.transport.as_mut())
    }

    pub fn set_source(&mut self, source: Source) -> Result<(), failure::Error> {
        SetSource::new(source).execute(self.transport.as_mut())
    }
}

pub fn get_minidsp_transport() -> Result<transport::HID, HidError> {
    let hid = HidApi::new().unwrap();
    // for device in hid.device_list() {
    //     println!("{:?} {:?} {:?} {:?}", device.vendor_id(), device.product_id(), device.manufacturer_string(), device.product_string())
    // }
    let (vid, pid) = (0x2752, 0x0011);
    let hid_device = hid.open(vid, pid)?;
    Ok(transport::HID::new(hid_device))
}
