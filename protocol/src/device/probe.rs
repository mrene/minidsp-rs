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
    #[cfg(feature = "device_ddrc24")]
    DDRC24,
    #[cfg(feature = "device_nanodigi2x8")]
    Nanodigi2x8,
}

impl Default for DeviceKind {
    fn default() -> Self {
        DeviceKind::Generic
    }
}

/// Attempts to get a `&Device` from a DeviceInfo
pub fn probe(device_info: &DeviceInfo) -> &'static super::Device {
    by_kind(probe_kind(device_info))
}

pub fn probe_kind(device_info: &DeviceInfo) -> DeviceKind {
    use DeviceKind::*;
    match (device_info.hw_id, device_info.dsp_version) {
        #[cfg(feature = "device_4x10hd")]
        (1, _) => M4x10Hd,
        #[cfg(feature = "device_msharc4x8")]
        (4, _) => MSharc4x8,
        #[cfg(feature = "device_2x4hd")]
        (10, 100) => M2x4Hd,
        #[cfg(feature = "device_ddrc24")]
        (10, 101) => DDRC24,
        #[cfg(feature = "device_shd")]
        (14, _) => Shd,
        #[cfg(feature = "device_nanodigi2x8")]
        (9, 54) => Nanodigi2x8,
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

        #[cfg(feature = "device_ddrc24")]
        DDRC24 => &super::ddrc24::DEVICE,

        #[cfg(feature = "device_shd")]
        Shd => &super::shd::DEVICE,

        #[cfg(feature = "device_nanodigi2x8")]
        Nanodigi2x8 => &super::nanodigi2x8::DEVICE,
    }
}
