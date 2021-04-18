//! Resolves a `Device` struct using a hardware id + dsp version

use super::{m2x4hd::DEVICE as DEVICE_2X4HD, Device};
use crate::DeviceInfo;

/// Attempts to get a `&Device` from a DeviceInfo
/// Returns None if no devices match
pub fn probe(device_info: &DeviceInfo) -> Option<&'static Device> {
    match device_info.hw_id {
        10 => Some(&DEVICE_2X4HD),
        _ => None,
    }
}
