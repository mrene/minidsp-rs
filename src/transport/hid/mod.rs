use anyhow::Result;
pub use handle::HidTransport;
use hidapi::{HidApi, HidError};
mod async_wrapper;
pub mod handle;

pub fn find_minidsp(vid: Option<u16>, pid: Option<u16>) -> Result<HidTransport, HidError> {
    let vid = vid.unwrap_or(0x2752);
    let pid = pid.unwrap_or(0x0011);

    let hid = HidApi::new()?;
    let hid_device = hid.open(vid, pid)?;
    Ok(HidTransport::new(hid_device))
}
