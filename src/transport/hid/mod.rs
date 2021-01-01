//! HID transport for local USB devices
use crate::transport::{MiniDSPError, Openable};
use anyhow::anyhow;
use anyhow::Result;
use async_trait::async_trait;
pub use handle::HidTransport;
use hidapi::{HidApi, HidError};
use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;

mod async_wrapper;
pub mod handle;

pub const VID_MINIDSP: u16 = 0x2752;

#[derive(Debug, Clone)]
pub struct Device {
    pub id: Option<(u16, u16)>,
    pub path: Option<String>,
}

impl FromStr for Device {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(path) = s.strip_prefix("path=") {
            Ok(Device {
                id: None,
                path: Some(path.to_owned()),
            })
        } else {
            let parts: Vec<_> = s.split(':').collect();
            if parts.len() != 2 {
                return Err("expected: vid:pid or path=...");
            }

            let vendor_id =
                u16::from_str_radix(parts[0], 16).map_err(|_| "couldn't parse vendor id")?;
            let product_id =
                u16::from_str_radix(parts[1], 16).map_err(|_| "couldn't parse product id")?;
            Ok(Device {
                id: Some((vendor_id, product_id)),
                path: None,
            })
        }
    }
}
impl fmt::Display for Device {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let id = match self.id {
            Some((vid, pid)) => format!("{:04x}:{:04x}", vid, pid),
            None => "unknown".to_owned(),
        };

        let path = match &self.path {
            Some(path) => format!("path={}", path),
            None => "".to_owned(),
        };

        write!(f, "[{}] {}", id, path,)
    }
}

#[async_trait]
impl Openable for Device {
    type Transport = HidTransport;

    async fn open(&self) -> Result<Self::Transport, MiniDSPError> {
        if let Some(path) = &self.path {
            Ok(HidTransport::with_path(path.to_string())?)
        } else if let Some((vid, pid)) = &self.id {
            Ok(HidTransport::with_product_id(*vid, *pid)?)
        } else {
            Err(MiniDSPError::InternalError(anyhow!(
                "invalid device, no path or id"
            )))
        }
    }
}

pub fn discover() -> Result<Vec<Device>, HidError> {
    let hid = HidApi::new()?;
    Ok(hid
        .device_list()
        .filter(|di| di.vendor_id() == VID_MINIDSP)
        .map(|di| Device {
            id: Some((di.vendor_id(), di.product_id())),
            path: Some(di.path().to_string_lossy().to_string()),
        })
        .collect())
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use hidapi::HidApi;

    #[test]
    fn discover() -> Result<()> {
        let hid = HidApi::new()?;
        for device in hid.device_list() {
            println!(
                "{:04x} {:04x} {}",
                device.vendor_id(),
                device.product_id(),
                device.path().to_string_lossy()
            );
        }
        Ok(())
    }
}
