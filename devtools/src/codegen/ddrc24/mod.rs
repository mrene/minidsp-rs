use bimap::BiHashMap;
use minidsp::formats::xml_config::Setting;
use strong_xml::XmlRead;

use super::spec::*;

pub struct Target {}
impl crate::Target for Target {
    fn filename() -> &'static str {
        "ddrc24.rs"
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
        product_name: "DDRC-24".into(),
        sources: vec!["Analog".into(), "Toslink".into(), "Usb".into()],
        inputs: vec![],
        outputs: (0..4).map(output).collect(),
        fir_max_taps: 0,
        internal_sampling_rate: 96000,
    }
}

pub fn symbols() -> BiHashMap<String, usize> {
    let cfg = include_str!("config.xml");
    Setting::from_str(cfg).unwrap().name_map()
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_codegen() {
        let mut symbol_map = symbols();
        let spec = device();
        super::super::generate_static_config(&mut symbol_map, &spec).to_string();
    }

    #[test]
    fn test_debug() {
        // Values provided from https://github.com/mrene/minidsp-rs/issues/115
        let dump = r#"
        0008: -10.3229065
        000d: -10.3229065
        0010: -120.0
        0011: -120.0
        0012: -120.0
        0013: -120.0
        0014: -120.0
        0015: -120.0
        0016: -120.0
        0017: -120.0
        0018: -120.0
        0019: -120.0
        001a: -120.0
        001b: -120.0
        001c: -120.0
        001d: -120.0
        001e: -120.0
        001f: -120.0
        0020: 0.000000000000000000000000000000000000000000003
        0021: 0.000000000000000000000000000000000000000000001
        0023: -30.0
        002a: -24.062094
        002c: -66.258286
        0060: 0.0038172458
        0061: 0.0076344917
        0062: 0.0038172458
        0063: 0.0076344917
        0064: 0.0038172458
        0065: 1.7695043
        0066: -0.78477335
        0067: 0.0038172458
        0068: 0.0076344917
        0069: 0.0038172458
        006a: 1.7695043
        006b: -0.78477335
        006c: 0.0040740687
        006d: 0.0081481375
        006e: 0.0040740687
        006f: 1.888556
        0070: 0.0040740687
        0071: 0.0081481375
        0072: 0.0040740687
        0073: 1.888556
        0074: -0.9048522
        0075: 0.8885694
        0076: -1.7771388
        0077: 0.8885694
        0078: 1.7695043
        0079: -0.78477335
        007a: 0.8885694
        007b: -1.7771388
        007c: 0.8885694
        007d: 1.7695043
        007e: 0.94835204
        007f: -1.8967041
        0080: 0.94835204
        0081: 1.888556
        0082: -0.9048522
        0083: 0.94835204
        0084: -1.8967041
        0085: 0.94835204
        0086: 1.888556
        0087: -0.9048522
        0088: 0.000000000000000000000000000000000000000000003
        0089: 0.000000000000000000000000000000000000000000001
        008b: -30.0
        0092: -21.3014
        0094: -62.289444
        00c8: 0.06613022
        00c9: 0.13226044
        00ca: 0.06613022
        00cb: 1.0155429
        00cc: -0.28006372
        00cd: 0.06613022
        00ce: 0.13226044
        00cf: 0.06613022
        00d0: 1.0155429
        00d1: -0.28006372
        00d2: 0.083800845
        00d3: 0.16760169
        00d4: 0.083800845
        00d5: 1.2869054
        00d6: -0.6221088
        00d7: 0.083800845
        00d8: 0.16760169
        00d9: 0.083800845
        00da: 1.2869054
        00db: -0.6221088
        00dc: 0.8885694
        00dd: -1.7771388
        00de: 0.8885694
        00df: 1.7695043
        00e0: 0.8885694
        00e1: -1.7771388
        00e2: 0.8885694
        00e3: 1.7695043
        00e4: -0.78477335
        00e5: 0.8885694
        00e6: -1.7771388
        00e7: 0.8885694
        00e8: 1.7695043
        00e9: -0.78477335
        00ea: 0.94835204
        00eb: -1.8967041
        00ec: 0.94835204
        00ed: 1.888556
        00ee: 0.94835204
        00ef: -1.8967041
        00f0: 0.94835204
        00f1: 1.888556
        00f2: -0.9048522
        00f3: -30.0
        00fa: -120.0
        00fb: NaN
        00fc: -120.0
"#;

        let syms = symbols();
        let s: Vec<_> = dump
            .lines()
            .map(str::trim)
            .filter_map(|s| {
                let mut split = s.split(": ");
                let addr = split.next()?;
                let value = split.next()?;

                let addr = u16::from_str_radix(addr, 16).ok()?;
                let addr = syms.get_by_right(&(addr as usize)).map(|s| s.as_str())?;
                let value: f32 = value.parse().ok()?;

                Some((addr, value))
            })
            .collect();
        println!("{:#?}", s)
    }
}
