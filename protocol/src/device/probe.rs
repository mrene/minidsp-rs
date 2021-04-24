//! Resolves a `Device` struct using a hardware id + dsp version

use crate::DeviceInfo;

/// Attempts to get a `&Device` from a DeviceInfo
/// Returns None if no devices match
pub fn probe(device_info: &DeviceInfo) -> Option<&'static super::Device> {
    match device_info.hw_id {
        #[cfg(feature = "device_msharc4x8")]
        4 => Some(&super::msharc4x8::DEVICE),

        #[cfg(feature = "device_2x4hd")]
        10 => Some(&super::m2x4hd::DEVICE),

        _ => Some(&super::GENERIC),
    }
}
