//! HID transport for local USB devices
use crate::transport::Transport;
use anyhow::Result;
use atomic_refcell::AtomicRefCell;
use frame_codec::FrameCodec;
use futures::{SinkExt, StreamExt};
use hidapi::{HidApi, HidDevice, HidError, HidResult};
use std::sync::Arc;
use std::{convert::Infallible, ops::Deref};
use stream::HidStream;

mod discover;
mod stream;
mod wrapper;
pub use discover::*;

use super::{frame_codec, multiplexer::Multiplexer};

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
    pub fn new(device: HidDevice) -> Arc<Multiplexer> {
        let stream = HidStream::new(device);
        let framed = FrameCodec::new(stream);
        // let framed = rx.map(|bytes| async move { Ok::<_, Infallible>(bytes) });
        let (tx, rx) = framed.split();
        Multiplexer::new(Box::pin(rx), Box::pin(tx.sink_err_into()))
    }

    pub fn with_path(hid: &HidApi, path: String) -> Result<Arc<Multiplexer>, HidError> {
        let path = std::ffi::CString::new(path.into_bytes()).unwrap();
        Ok(HidTransport::new(hid.open_path(&path)?))
    }

    pub fn with_product_id(hid: &HidApi, vid: u16, pid: u16) -> Result<Arc<Multiplexer>, HidError> {
        let hid_device = hid.open(vid, pid)?;
        Ok(HidTransport::new(hid_device))
    }
}
