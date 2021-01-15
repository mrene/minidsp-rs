//! HID transport for local USB devices
use crate::transport::Transport;
use anyhow::Result;
use atomic_refcell::AtomicRefCell;
use futures::{SinkExt, StreamExt};
use hidapi::{HidApi, HidDevice, HidError, HidResult};
use std::ops::Deref;
use std::sync::Arc;
use stream::HidStream;

mod discover;
mod stream;
mod wrapper;
pub use discover::*;

pub const VID_MINIDSP: u16 = 0x2752;

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

pub struct HidTransport {}

impl HidTransport {
    pub fn new(device: HidDevice) -> Transport {
        let stream = HidStream::new(device);
        let (tx, rx) = stream.split();
        Transport::new(Box::pin(rx), Box::pin(tx.sink_err_into()))
    }

    pub fn with_path(hid: &HidApi, path: String) -> Result<Transport, HidError> {
        let path = std::ffi::CString::new(path.into_bytes()).unwrap();
        Ok(HidTransport::new(hid.open_path(&path)?))
    }

    pub fn with_product_id(hid: &HidApi, vid: u16, pid: u16) -> Result<Transport, HidError> {
        let hid_device = hid.open(vid, pid)?;
        Ok(HidTransport::new(hid_device))
    }
}
