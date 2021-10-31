use bimap::BiHashMap;
use minidsp::formats::xml_config::Setting;
use strong_xml::XmlRead;

use super::spec::*;

pub struct Target {}
impl crate::Target for Target {
    fn filename() -> &'static str {
        "msharc4x8.rs"
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
            gain: Some(format!("DGain_{}_0", input + 1)),
        }),
        meter: Some(format!("Meter04_C1_{}", input)),
        peq: (0..10usize)
            .map(|index| format!("PEQ_{}_{}", input + 1, 10 - index))
            .collect(),
        routing: (0..4usize)
            .map(|output| Gate {
                enable: format!("MixerNxMSmoothed1_{}_{}_status", input, output),
                gain: Some(format!("MixerNxMSmoothed1_{}_{}", input, output)),
            })
            .collect(),
    }
}

pub(crate) fn output(output: usize) -> Output {
    let channel_index = output + 5;
    Output {
        gate: Gate {
            enable: format!("DGain_{}_0_status", channel_index),
            gain: Some(format!("DGain_{}_0", channel_index)),
        },
        meter: Some(format!("Meter10_C2_{}", output)),
        delay_addr: Some(format!("Delay_{}_0", channel_index)),
        invert_addr: format!("polarity_out_1_{}", 4 + output),
        peq: (0..10usize)
            .map(|index| format!("PEQ_{}_{}", channel_index, 10 - index))
            .collect(),
        xover: Some(Crossover {
            peqs: [1, 5]
                .iter()
                .map(|group| format!("BPF_{}_{}", channel_index, group))
                .collect(),
        }),
        compressor: Some(Compressor {
            bypass: format!("COMP_{}_0_status", channel_index),
            threshold: format!("COMP_{}_0_threshold", channel_index),
            ratio: format!("COMP_{}_0_ratio", channel_index),
            attack: format!("COMP_{}_0_atime", channel_index),
            release: format!("COMP_{}_0_rtime", channel_index),
            meter: Some(format!("Meter10_C1_{}", output)),
        }),
        fir: Some(Fir {
            index: output as u8,
            num_coefficients: format!("FIR_{}_0_Taps", channel_index),
            bypass: format!("FIR_{}_0_status", channel_index),
            max_coefficients: 2048,
        }),
    }
}

pub fn device() -> Device {
    Device {
        product_name: "MiniSHARC 4x8".into(),
        sources: Vec::new(),
        inputs: (0..4).map(input).collect(),
        outputs: (0..8).map(output).collect(),
        fir_max_taps: 9600,

        // FIXME: This depends on the installed plugin
        internal_sampling_rate: 96000,
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
    let s = super::generate_static_config(&mut symbol_map, &spec).to_string();
    assert!(s.len() > 1000);
}
