//! HID transport for local USB devices
use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use atomic_refcell::AtomicRefCell;
use frame_codec::FrameCodec;
use futures::{SinkExt, TryStreamExt};
pub use hidapi::HidError;
use hidapi::{HidApi, HidDevice, HidResult};
use stream::HidStream;
use url2::Url2;

mod discover;
mod stream;
mod wrapper;
pub use discover::*;

use super::{frame_codec, multiplexer::Multiplexer, IntoTransport};

pub const VID_MINIDSP: u16 = 0x2752;
pub const OLD_MINIDSP_PID: (u16, u16) = (0x04d8, 0x003f);

static HIDAPI_INSTANCE: AtomicRefCell<Option<Arc<Mutex<HidApi>>>> = AtomicRefCell::new(None);

/// Initializes a global instance of HidApi
pub fn initialize_api() -> HidResult<Arc<Mutex<HidApi>>> {
    if let Some(x) = HIDAPI_INSTANCE.borrow().deref() {
        return Ok(x.clone());
    }

    let api = Arc::new(Mutex::new(HidApi::new()?));
    HIDAPI_INSTANCE.borrow_mut().replace(api.clone());
    Ok(api)
}

pub struct HidTransport {
    stream: HidStream,
}

impl HidTransport {
    pub fn new(device: HidDevice) -> HidTransport {
        HidTransport {
            stream: HidStream::new(device),
        }
    }

    pub fn with_url(hid: &HidApi, url: &Url2) -> Result<HidTransport, HidError> {
        // If we have a path, decode it.
        let path = url.path();
        if !path.is_empty() {
            let path = urlencoding::decode(path).map_err(|_| HidError::OpenHidDeviceError)?;
            return HidTransport::with_path(hid, path.to_string());
        }

        // If it's empty, try to get the vid and pid from the query string
        let query: HashMap<_, _> = url.query_pairs().collect();
        let vid = query.get("vid");
        let pid = query.get("pid");

        if let (Some(vid), Some(pid)) = (vid, pid) {
            let vid = u16::from_str_radix(vid, 16).map_err(|_| HidError::HidApiError {
                message: "couldn't parse vendor id".to_string(),
            })?;
            let pid = u16::from_str_radix(pid, 16).map_err(|_| HidError::HidApiError {
                message: "couldn't parse product id".to_string(),
            })?;
            return HidTransport::with_product_id(hid, vid, pid);
        }

        Err(HidError::HidApiError {
            message: "malformed url".to_string(),
        })
    }

    pub fn with_path(hid: &HidApi, path: String) -> Result<HidTransport, HidError> {
        let path = std::ffi::CString::new(path.into_bytes()).unwrap();
        Ok(HidTransport::new(hid.open_path(&path)?))
    }

    pub fn with_product_id(hid: &HidApi, vid: u16, pid: u16) -> Result<HidTransport, HidError> {
        let hid_device = hid.open(vid, pid)?;
        Ok(HidTransport::new(hid_device))
    }

    pub fn into_multiplexer(self) -> Arc<Multiplexer> {
        Multiplexer::new(FrameCodec::new(self.stream).sink_err_into().err_into())
    }

    pub fn into_inner(self) -> HidStream {
        self.stream
    }
}

impl IntoTransport for HidTransport {
    fn into_transport(self) -> super::Transport {
        Box::pin(self.into_inner().sink_err_into().err_into())
    }
}
