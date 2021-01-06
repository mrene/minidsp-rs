//! Utilities for dealing with xml configuration files
use strong_xml::{XmlRead, XmlWrite};

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
#[xml(tag = "setting")]
pub struct Setting {
    #[xml(attr = "version")]
    pub version: String,

    #[xml(flatten_text = "timestamp")]
    pub timestamp: u32,

    #[xml(flatten_text = "dspversion")]
    pub dsp_version: u8,

    #[xml(flatten_text = "master_mute")]
    pub master_mute: u8,

    #[xml(child = "label")]
    pub labels: Vec<Label>,

    #[xml(child = "item")]
    pub items: Vec<Item>,

    #[xml(child = "fir")]
    pub fir: Vec<Fir>,

    #[xml(child = "filter")]
    pub filter: Vec<Filter>,

    #[xml(flatten_text = "link")]
    pub link: String,
}

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
#[xml(tag = "label")]
pub struct Label {
    #[xml(attr = "name")]
    pub name: String,
    #[xml(attr = "text")]
    pub text: String,
}

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
#[xml(tag = "item")]
pub struct Item {
    #[xml(attr = "name")]
    pub name: String,
    #[xml(attr = "addr")]
    pub addr: String,
    #[xml(flatten_text = "dec")]
    pub dec: String,
    #[xml(flatten_text = "hex")]
    pub hex: String,
}

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
#[xml(tag = "fir")]
pub struct Fir {
    #[xml(attr = "name")]
    pub name: String,
    #[xml(attr = "addr")]
    pub addr: u16,
    #[xml(child = "subpara")]
    pub subpara: Vec<Subpara>,
}

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
#[xml(tag = "subpara")]
pub struct Subpara {
    #[xml(attr = "row")]
    pub row: u8,
    #[xml(text)]
    pub data: String,
}

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
#[xml(tag = "filter")]
pub struct Filter {
    #[xml(attr = "name")]
    pub row: String,
    #[xml(attr = "addr")]
    pub addr: String,

    #[xml(flatten_text = "freq")]
    pub freq: u16,
    #[xml(flatten_text = "q")]
    pub q: f32,
    #[xml(flatten_text = "boost")]
    pub boost: f32,
    #[xml(flatten_text = "type")]
    pub typ: String,
    #[xml(flatten_text = "bypass")]
    pub bypass: u8,
    #[xml(flatten_text = "dec")]
    pub dec: String,
    #[xml(flatten_text = "hex")]
    pub hex: String,
}

mod test {
    use super::*;

    #[test]
    fn test_deserialize() {
        let file = include_str!("./test_fixtures/config.xml");
        let root = Setting::from_str(file);
        println!("{:#?}", root);
    }
}
