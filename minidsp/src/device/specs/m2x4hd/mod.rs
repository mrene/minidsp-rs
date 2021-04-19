use strong_xml::XmlRead;

use super::{CompressorSpec, DeviceSpec, FirSpec, SymbolMap};
use crate::xml_config::Setting;

pub struct Spec {
    pub sym_map: SymbolMap,
}

impl Spec {
    pub fn new() -> Self {
        let cfg = include_str!("config.xml");
        Self {
            sym_map: Setting::from_str(cfg).unwrap().name_map(),
        }
    }
}

impl Default for Spec {
    fn default() -> Self {
        Spec::new()
    }
}

impl DeviceSpec for Spec {
    fn product_name(&self) -> String {
        "2x4HD".to_owned()
    }

    fn symbol_map(&mut self) -> &mut SymbolMap {
        &mut self.sym_map
    }

    fn sources_names(&self) -> Vec<String> {
        vec![
            "Analog".to_string(),
            "Toslink".to_string(),
            "Usb".to_string(),
        ]
    }

    fn num_inputs(&self) -> usize {
        2
    }

    fn num_outputs(&self) -> usize {
        4
    }

    fn routing_enable(&self, input: usize, output: usize) -> String {
        format!("MixerNxMSmoothed1_{}_{}_status", input, output)
    }

    fn routing_gain(&self, input: usize, output: usize) -> String {
        format!("MixerNxMSmoothed1_{}_{}", input, output)
    }

    fn input_meter(&self, input: usize) -> String {
        format!("Meter02_C1_{}", input)
    }

    fn input_enable(&self, input: usize) -> String {
        format!("DGain_{}_0_status", input + 1)
    }

    fn input_gain(&self, input: usize) -> String {
        format!("DGain_{}_0", input + 1)
    }

    fn input_num_peq(&self) -> usize {
        10
    }

    fn input_peq(&self, input: usize, index: usize) -> String {
        format!("PEQ_{}_{}", input + 1, 10 - index)
    }

    fn output_meter(&self, output: usize) -> String {
        format!("Meter10_C1_{}", 4 + output)
    }

    fn output_enable(&self, output: usize) -> String {
        format!("DGain_{}_0_status", 3 + output)
    }

    fn output_gain(&self, output: usize) -> String {
        format!("DGain_{}_0", 3 + output)
    }

    fn output_delay(&self, output: usize) -> String {
        format!("Delay_{}_0", 3 + output)
    }

    fn output_invert(&self, output: usize) -> String {
        format!("polarity_out_{}_0", 1 + output)
    }

    fn output_num_peq(&self) -> usize {
        10
    }

    fn output_peq(&self, output: usize, index: usize) -> String {
        format!("PEQ_{}_{}", output + 3, 10 - index)
    }

    fn output_xover(&self, output: usize, group: usize) -> String {
        let group = match group {
            0 => 1,
            1 => 5,
            _ => panic!("more than 2 groups"),
        };
        format!("BPF_{}_{}", output + 3, group)
    }

    fn output_compressor(&self, output: usize) -> CompressorSpec {
        CompressorSpec {
            threshold: format!("COMP_{}_0_threshold", output + 3),
            bypass: format!("COMP_{}_0_status", output + 3),
            ratio: format!("COMP_{}_0_ratio", output + 3),
            attack: format!("COMP_{}_0_atime", output + 3),
            release: format!("COMP_{}_0_rtime", output + 3),
            meter: format!("Meter10_C1_{}", output),
        }
    }

    fn output_fir(&self, output: usize) -> FirSpec {
        FirSpec {
            index: output,
            bypass: format!("FIR_{}_0_status", output + 3),
            num_coefficients: format!("FIR_{}_0_Taps", output + 3),
            max_coefficients: 4096,
        }
    }

    fn fir_max_taps(&self) -> usize {
        4096
    }

    fn internal_sampling_rate(&self) -> u32 {
        96000
    }
}
