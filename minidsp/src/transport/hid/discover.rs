//! Discovery of local devices
use std::{fmt, fmt::Formatter, ops::Deref, str::FromStr};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hidapi::{HidApi, HidError};

use super::{initialize_api, HidTransport, OLD_MINIDSP_PID, VID_MINIDSP};
use crate::transport::{IntoTransport, MiniDSPError, Openable, Transport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: Option<(u16, u16)>,
    pub path: Option<String>,
}

impl Device {
    pub fn to_url(&self) -> String {
        ToString::to_string(&self)
    }
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
        let query = match self.id {
            Some((vid, pid)) => format!("?vid={:04x}&pid={:04x}", vid, pid),
            None => "".to_owned(),
        };

        let path = match &self.path {
            Some(path) => urlencoding::encode(path.as_str()),
            None => "".to_owned(),
        };

        write!(f, "usb:{}{}", path, query)
    }
}

#[async_trait]
impl Openable for Device {
    async fn open(&self) -> Result<Transport, MiniDSPError> {
        if let Some(path) = &self.path {
            Ok(
                HidTransport::with_path(initialize_api()?.deref(), path.to_string())?
                    .into_transport(),
            )
        } else if let Some((vid, pid)) = &self.id {
            Ok(
                HidTransport::with_product_id(initialize_api()?.deref(), *vid, *pid)?
                    .into_transport(),
            )
        } else {
            Err(MiniDSPError::InternalError(anyhow!(
                "invalid device, no path or id"
            )))
        }
    }

    fn to_string(&self) -> String {
        ToString::to_string(self)
    }
}

pub fn discover(hid: &HidApi) -> Result<Vec<Device>, HidError> {
    Ok(hid
        .device_list()
        .filter(|di| {
            di.vendor_id() == VID_MINIDSP || (di.vendor_id(), di.product_id()) == OLD_MINIDSP_PID
        })
        .map(|di| Device {
            id: Some((di.vendor_id(), di.product_id())),
            path: Some(di.path().to_string_lossy().to_string()),
        })
        .collect())
}

pub fn discover_with<F: Fn(&hidapi::DeviceInfo) -> bool>(
    hid: &HidApi,
    func: F,
) -> Result<Vec<Device>, HidError> {
    Ok(hid
        .device_list()
        .filter(|di| func(di))
        .map(|di| Device {
            id: Some((di.vendor_id(), di.product_id())),
            path: Some(di.path().to_string_lossy().to_string()),
        })
        .collect())
}
