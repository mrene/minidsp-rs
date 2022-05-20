//! Utilities to get a mapping from the source name to the source id
//! Most of this logic was translated from the cordova app

use super::DeviceInfo;

#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "debug", derive(Debug))]
#[cfg_attr(
    feature = "use_serde",
    derive(
        strum::EnumString,
        strum::Display,
        serde::Serialize,
        serde::Deserialize,
        schemars::JsonSchema,
    )
)]
#[cfg_attr(feature = "use_serde", strum(serialize_all = "lowercase"))]
pub enum Source {
    NotInstalled,
    Analog,
    Toslink,
    Spdif,
    Usb,
    Aesebu,
    Rca,
    Xlr,
    Lan,
    I2S,
    Bluetooth,
}

impl Source {
    /// Gets the available input sources with their numerical mapping for a given device
    pub fn mapping(device_info: &DeviceInfo) -> &'static [(Source, u8)] {
        use Source::*;
        match device_info.hw_id {
            2 | 11 => &[(Toslink, 0), (Spdif, 1)],
            1 | 4 | 5 => &[(Spdif, 0), (Toslink, 1), (Aesebu, 2)],
            10 if device_info.dsp_version == 100 || device_info.dsp_version == 101 => {
                &[(Analog, 0), (Toslink, 1), (Usb, 2)]
            }
            10 => &[(I2S, 0), (Toslink, 1), (Usb, 2)],
            14 => &[
                (Toslink, 0),
                (Spdif, 1),
                (Aesebu, 2),
                (Rca, 3),
                (Xlr, 4),
                (Usb, 5),
                (Lan, 6),
            ],
            17 | 18 => &[(Toslink, 0), (Spdif, 1), (Aesebu, 2), (Usb, 3), (Lan, 4)],
            27 => &[(Analog, 0), (Toslink, 1), (Spdif, 2), (Usb, 3), (Bluetooth, 4)],
            _ => &[(NotInstalled, 0)],
        }
    }

    pub fn from_id(id: u8, device_info: &DeviceInfo) -> Self {
        for (src, src_id) in Self::mapping(device_info) {
            if *src_id == id {
                return *src;
            }
        }
        Source::NotInstalled
    }

    pub fn to_id(self, device_info: &DeviceInfo) -> u8 {
        for &(src, src_id) in Self::mapping(device_info) {
            if src == self {
                return src_id;
            }
        }
        0
    }
}
