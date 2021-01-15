//! Discovery of local devices
use super::{initialize_api, HidTransport, VID_MINIDSP};
use crate::transport::{MiniDSPError, Openable, Transport};
use anyhow::anyhow;
use anyhow::Result;
use async_trait::async_trait;
use hidapi::{HidApi, HidError};
use std::fmt;
use std::fmt::Formatter;
use std::ops::Deref;
use std::str::FromStr;

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
    async fn open(&self) -> Result<Transport, MiniDSPError> {
        if let Some(path) = &self.path {
            Ok(HidTransport::with_path(
                initialize_api()?.deref(),
                path.to_string(),
            )?)
        } else if let Some((vid, pid)) = &self.id {
            Ok(HidTransport::with_product_id(
                initialize_api()?.deref(),
                *vid,
                *pid,
            )?)
        } else {
            Err(MiniDSPError::InternalError(anyhow!(
                "invalid device, no path or id"
            )))
        }
    }
}

pub fn discover(hid: &HidApi) -> Result<Vec<Device>, HidError> {
    Ok(hid
        .device_list()
        .filter(|di| di.vendor_id() == VID_MINIDSP)
        .map(|di| Device {
            id: Some((di.vendor_id(), di.product_id())),
            path: Some(di.path().to_string_lossy().to_string()),
        })
        .collect())
}
