use anyhow::Result;
pub use handle::HidTransport;
use hidapi::{HidApi, HidError};
mod async_wrapper;
pub mod handle;

pub fn find_minidsp() -> Result<HidTransport, HidError> {
    let hid = HidApi::new().unwrap();
    let (vid, pid) = (0x2752, 0x0011);
    let hid_device = hid.open(vid, pid)?;
    Ok(HidTransport::new(hid_device))
}
