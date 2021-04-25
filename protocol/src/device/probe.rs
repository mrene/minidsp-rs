//! Resolves a `Device` struct using a hardware id + dsp version

use crate::DeviceInfo;

#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "debug", derive(Debug))]
#[cfg_attr(
    feature = "use_serde",
    derive(
        strum::EnumString,
        strum::ToString,
        serde::Serialize,
        serde::Deserialize,
        schemars::JsonSchema,
    )
)]
#[cfg_attr(feature = "use_serde", strum(serialize_all = "lowercase"))]
pub enum DeviceKind {
    Generic,
    #[cfg(feature = "device_4x10hd")]
    M4x10Hd,
    #[cfg(feature = "device_msharc4x8")]
    MSharc4x8,
    #[cfg(feature = "device_2x4hd")]
    M2x4Hd,
    #[cfg(feature = "device_shd")]
    Shd,
}

/// Attempts to get a `&Device` from a DeviceInfo
pub fn probe(device_info: &DeviceInfo) -> &'static super::Device {
    by_kind(probe_kind(device_info))
}

pub fn probe_kind(device_info: &DeviceInfo) -> DeviceKind {
    use DeviceKind::*;
    match device_info.hw_id {
        #[cfg(feature = "device_4x10hd")]
        1 => M4x10Hd,
        #[cfg(feature = "device_msharc4x8")]
        4 => MSharc4x8,
        #[cfg(feature = "device_2x4hd")]
        10 => M2x4Hd,
        #[cfg(feature = "device_shd")]
        14 => Shd,
        _ => Generic,
    }
}

pub fn by_kind(kind: DeviceKind) -> &'static super::Device {
    use DeviceKind::*;
    match kind {
        Generic => &super::GENERIC,

        #[cfg(feature = "device_4x10hd")]
        M4x10Hd => &super::m4x10hd::DEVICE,

        #[cfg(feature = "device_msharc4x8")]
        MSharc4x8 => &super::msharc4x8::DEVICE,

        #[cfg(feature = "device_2x4hd")]
        M2x4Hd => &super::m2x4hd::DEVICE,

        #[cfg(feature = "device_shd")]
        Shd => &super::shd::DEVICE,
    }
}
