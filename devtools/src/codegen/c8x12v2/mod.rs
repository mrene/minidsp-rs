use bimap::BiHashMap;
use minidsp::formats::xml_config::Setting;
use strong_xml::XmlRead;

use super::spec::*;

pub struct Target {}
impl crate::Target for Target {
    fn filename() -> &'static str {
        "c8x12v2.rs"
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
            enable: format!("DGain_{}_0_status", input + 1),
            gain: format!("DGain_{}_0", input + 1),
        }),
        meter: Some(format!("Meter_In_{}", input + 1)),
        peq: (0..10usize)
            .map(|index| format!("PEQ_{}_{}", input + 1, 10 - index))
            .collect(),
        routing: (0..8usize)
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
            enable: format!("DGain_{}_0_status", 9 + output),
            gain: format!("DGain_{}_0", 9 + output),
        },
        meter: format!("Meter_Out_{}", output + 1),
        delay_addr: format!("Delay_{}_0", 9 + output),
        invert_addr: format!("polarity_out_{}_0", 9 + output),
        peq: (1..=10usize)
            .rev()
            .map(|index| format!("PEQ_{}_{}", output + 9, index))
            .collect(),
        xover: Some(Crossover {
            peqs: [1, 5]
                .iter()
                .map(|group| format!("BPF_{}_{}", output + 9, group))
                .collect(),
        }),
        compressor: Some(Compressor {
            bypass: format!("COMP_{}_0_status", output + 9),
            threshold: format!("COMP_{}_0_threshold", output + 9),
            ratio: format!("COMP_{}_0_ratio", output + 9),
            attack: format!("COMP_{}_0_atime", output + 9),
            release: format!("COMP_{}_0_rtime", output + 9),
            //gain: format!("COMP_{}_0_gain", output + 9),
            //knee: format!("COMP_{}_0_knee", output + 9),
            meter: Some(format!("Meter_Comp_{}", output + 1)),
        }),
        fir: None,
    }
}

pub fn device() -> Device {
    Device {
        product_name: "MiniDSP C-DSP 8x12 v2".into(),
        sources: vec!["Analog".into(), "Toslink".into(), "Spdif".into()],
        inputs: (0..8).map(input).collect(),
        outputs: (0..12).map(output).collect(),
        fir_max_taps: 0,
        internal_sampling_rate: 0,
        ..Default::default()
    }
}

pub fn symbols() -> BiHashMap<String, usize> {
    let cfg = include_str!("config.xml");
    Setting::from_str(cfg).unwrap().name_map()
}

#[cfg(test)]
#[test]
fn test_codegen() {
    let mut symbol_map = symbols();
    let spec = device();
    super::generate_static_config(&mut symbol_map, &spec).to_string();
}
