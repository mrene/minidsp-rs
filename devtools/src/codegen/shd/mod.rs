use bimap::BiHashMap;
use minidsp::formats::xml_config::Setting;
use strong_xml::XmlRead;

use super::spec::*;

pub struct Target {}
impl crate::Target for Target {
    fn filename() -> &'static str {
        "shd.rs"
    }

    fn symbols() -> bimap::BiMap<String, usize> {
        symbols()
    }

    fn device() -> Device {
        device()
    }
}

pub fn device() -> Device {
    Device {
        product_name: "MiniDSP SHD".into(),
        sources: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        fir_max_taps: 0,
        internal_sampling_rate: 0,
    }
}

pub fn symbols() -> BiHashMap<String, usize> {
    let cfg = include_str!("config.xml");
    Setting::from_str(cfg).unwrap().name_map()
}
