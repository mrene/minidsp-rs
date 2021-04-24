//! Resolves a `Device` struct using a hardware id + dsp version

#[cfg(feature = "device_2x4hd")]
use super::Device;

use crate::DeviceInfo;

/// Attempts to get a `&Device` from a DeviceInfo
/// Returns None if no devices match
pub fn probe(device_info: &DeviceInfo) -> Option<&'static Device> {
    match device_info.hw_id {
        #[cfg(feature = "device_2x4hd")]
        10 => Some(&crate::device::m2x4hd::DEVICE),
        _ => None,
    }
}
