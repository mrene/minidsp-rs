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

pub(crate) fn input(input: usize) -> Input {
    Input {
        gate: None,
        meter: Some(format!("Meter_D_In_{}", input + 1)),
        // No input PEQs
        peq: Vec::new(),
        routing: (0..4usize)
            .map(|output| Gate {
                enable: format!("Mixer_{}_{}_status", input, output),
                gain: format!("Mixer_{}_{}", input, output),
            })
            .collect(),
    }
}

pub(crate) fn output(output: usize) -> Output {
    Output {
        gate: Gate {
            enable: format!("DGain_{}_0_status", 1 + output),
            gain: format!("DGain_{}_0", 1 + output),
        },
        meter: format!("Meter_Out_{}", 1 + output),
        delay_addr: format!("Delay_{}_0", 1 + output),
        invert_addr: format!("polarity_out_{}_0", 1 + output),
        peq: (1..=10usize)
            .rev()
            .map(|index| format!("PEQ_{}_{}", output + 1, index))
            .collect(),
        xover: Some(Crossover {
            peqs: [1, 5]
                .iter()
                .map(|group| format!("BPF_{}_{}", output + 1, group))
                .collect(),
        }),
        compressor: Some(Compressor {
            bypass: format!("COMP_{}_0_status", output + 1),
            threshold: format!("COMP_{}_0_threshold", output + 1),
            ratio: format!("COMP_{}_0_ratio", output + 1),
            attack: format!("COMP_{}_0_atime", output + 1),
            release: format!("COMP_{}_0_rtime", output + 1),
            meter: Some(format!("Meter_Comp_{}", output + 1)),
        }),
        fir: None,
    }
}

pub fn device() -> Device {
    Device {
        product_name: "MiniDSP SHD".into(),
        sources: vec!["Analog".into(), "Toslink".into(), "Usb".into()],
        inputs: (0..2).map(input).collect(),
        outputs: (0..4).map(output).collect(),
        fir_max_taps: 0,
        internal_sampling_rate: 96000,
    }
}

pub fn symbols() -> BiHashMap<String, usize> {
    let cfg = include_str!("config.xml");
    Setting::from_str(cfg).unwrap().name_map()
}
