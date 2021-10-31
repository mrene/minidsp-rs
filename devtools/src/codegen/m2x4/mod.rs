use bimap::BiHashMap;
use minidsp::{formats::xml_config::Setting, AddrEncoding, Dialect, FloatEncoding};
use strong_xml::XmlRead;

use super::spec::*;

pub struct Target {}
impl crate::Target for Target {
    fn filename() -> &'static str {
        "m2x4.rs"
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
        product_name: "MiniDSP 2x4".into(),
        sources: vec!["Toslink".into(), "Spdif".into()],
        inputs: vec![],
        outputs: vec![],
        fir_max_taps: 0,
        internal_sampling_rate: 48000,
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
