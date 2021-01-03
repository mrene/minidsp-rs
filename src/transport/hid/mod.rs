//! HID transport for local USB devices
use crate::transport::{MiniDSPError, Openable};
use anyhow::anyhow;
use anyhow::Result;
use async_trait::async_trait;
use atomic_refcell::AtomicRefCell;
pub use handle::HidTransport;
use hidapi::{HidApi, HidError, HidResult};
use std::fmt;
use std::fmt::Formatter;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

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

static HIDAPI_INSTANCE: AtomicRefCell<Option<Arc<HidApi>>> = AtomicRefCell::new(None);

/// Initializes a global instance of HidApi
pub fn initialize_api() -> HidResult<Arc<HidApi>> {
    if let Some(x) = HIDAPI_INSTANCE.borrow().deref() {
        return Ok(x.clone());
    }

    let api = Arc::new(HidApi::new()?);
    HIDAPI_INSTANCE.borrow_mut().replace(api.clone());
    Ok(api)
}
