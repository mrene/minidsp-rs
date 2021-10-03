use bimap::BiHashMap;
use minidsp::{formats::xml_config::Setting, AddrEncoding, Dialect, FloatEncoding};
use strong_xml::XmlRead;

use super::spec::*;

pub struct Target {}

impl crate::Target for Target {
    fn filename() -> &'static str {
        "m4x10hd.rs"
    }

    fn symbols() -> bimap::BiMap<String, usize> {
        symbols()
    }

    fn device() -> Device {
        device()
    }
}

pub(crate) fn input(input: usize) -> Input {
    Input {
        gate: Some(Gate {
            enable: format!("MuteNoSlewAlg7{}mute", input + 1),
            gain: format!("Gain1940AlgNS{}", input + 11),
        }),
        meter: None,
        peq: (0..5usize)
            .map(|index| format!("PEQ_{}_{}", input + 11, 5 - index))
            .collect(),
        routing: Default::default(),
        // (0..4usize)
        //     .map(|output| Gate {
        //         enable: format!("MixerNxMSmoothed1_{}_{}_status", input, output),
        //         gain: String::new(),
        //     })
        //     .collect(),
    }
}

pub(crate) fn output(output: usize) -> Output {
    Output {
        gate: Gate {
            enable: format!("MuteNoSlewAlg{}mute", output + 1),
            gain: format!("Gain1940AlgNS{}", output + 1),
        },
        meter: String::new(),
        delay_addr: if output < 8 {
            format!("MultCtrlDelGrowAlg{}", output + 1)
        } else {
            String::new()
        },
        invert_addr: format!("EQ1940Invert{}gain", output + 1),
        peq: (1..=5usize)
            .rev()
            .map(|index| format!("PEQ_{}_{}", output + 1, index))
            .collect(),
        xover: Some(Crossover {
            peqs: [1, 5]
                .iter()
                .map(|group| format!("BPF_{}_{}", output + 1, group))
                .collect(),
        }),
        compressor: None,
        fir: None,
    }
}

pub fn device() -> Device {
    Device {
        product_name: "MiniDSP 4x10HD".into(),
        sources: vec!["Spdif".into(), "Toslink".into(), "Aesebu".into()],
        inputs: (0..4).map(input).collect(),
        outputs: (0..10).map(output).collect(),
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
