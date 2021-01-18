//! HID transport for local USB devices
use anyhow::Result;
use atomic_refcell::AtomicRefCell;
use frame_codec::FrameCodec;
use futures::{SinkExt, TryStreamExt};
use hidapi::{HidApi, HidDevice, HidError, HidResult};
use std::{ops::Deref, sync::Arc};
use stream::HidStream;

mod discover;
mod stream;
mod wrapper;
pub use discover::*;

use super::{frame_codec, multiplexer::Multiplexer, IntoTransport};

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

pub struct HidTransport {
    stream: HidStream,
}

impl HidTransport {
    pub fn new(device: HidDevice) -> HidTransport {
        HidTransport {
            stream: HidStream::new(device),
        }
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
