use bimap::BiHashMap;
use minidsp::{formats::xml_config::Setting, AddrEncoding, Dialect, FloatEncoding};
use strong_xml::XmlRead;

use super::{m4x10hd, spec::*};

pub struct Target {}
impl crate::Target for Target {
    fn filename() -> &'static str {
        "nanodigi2x8.rs"
    }

    fn symbols() -> bimap::BiMap<String, usize> {
        symbols()
    }

    fn device() -> Device {
        device()
    }
}

pub fn device() -> Device {
    #[allow(clippy::needless_update)]
    Device {
        product_name: "NanoDigi 2x8".into(),
        sources: vec!["Toslink".into(), "Spdif".into()],
        inputs: (0..2).map(|n| m4x10hd::input(n, 8)).collect(),
        outputs: (0..8).map(m4x10hd::output).collect(),
        fir_max_taps: 0,
        internal_sampling_rate: 96000,
        dialect: Dialect {
            addr_encoding: AddrEncoding::AddrLen2,
            float_encoding: FloatEncoding::FixedPoint,
        },
        ..Default::default()
    }
}

pub fn symbols() -> BiHashMap<String, usize> {
    let cfg = include_str!("config.xml");
    Setting::from_str(cfg).unwrap().name_map()
}
