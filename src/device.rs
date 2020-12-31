pub const DEVICE_2X4HD: Device = Device {
    source_names: &["Analog", "TOSLINK", "USB"],
    num_inputs: 2,
    num_outputs: 4,
    inputs: &[
        Input {
            index: 0,
            gate: Gate {
                enable: 0x00,
                gain: 0x1a,
            },
            peq: &[0x85, 0x80, 0x7b, 0x76, 0x71, 0x6c, 0x67, 0x62, 0x5d, 0x58],
            routing: &[
                Gate {
                    enable: 0x06,
                    gain: 0x20,
                },
                Gate {
                    enable: 0x07,
                    gain: 0x21,
                },
                Gate {
                    enable: 0x08,
                    gain: 0x22,
                },
                Gate {
                    enable: 0x09,
                    gain: 0x23,
                },
            ],
        },
        Input {
            index: 1,
            gate: Gate {
                enable: 0x01,
                gain: 0x1b,
            },
            peq: &[0xb7, 0xb2, 0xad, 0xa8, 0xa3, 0x9e, 0x99, 0x94, 0x8f, 0x8a],
            routing: &[
                Gate {
                    enable: 0x0a,
                    gain: 0x24,
                },
                Gate {
                    enable: 0x0b,
                    gain: 0x25,
                },
                Gate {
                    enable: 0x0c,
                    gain: 0x26,
                },
                Gate {
                    enable: 0x0d,
                    gain: 0x27,
                },
            ],
        },
    ],
    outputs: &[Output {
        index: 0,
        gate: Gate { enable: 0, gain: 0 },
        delay_addr: 0,
        invert_addr: 0,
        peq: &[],
        xover: &[],
        fir_coeff_addr: 0,
        fir_bypass_addr: 0,
    }],
};

pub struct Device {
    pub source_names: &'static [&'static str],
    pub num_inputs: usize,
    pub num_outputs: usize,
    pub inputs: &'static [Input],
    pub outputs: &'static [Output],
}

pub struct Input {
    pub index: usize,
    pub gate: Gate,
    pub peq: &'static [u8],
    pub routing: &'static [Gate],
}

pub struct Output {
    pub index: usize,
    pub gate: Gate,
    pub delay_addr: u16,
    pub invert_addr: u16,
    pub peq: &'static [u16],
    pub xover: &'static [u16],
    // TODO: Compressor
    pub fir_coeff_addr: u16,
    pub fir_bypass_addr: u16,
}

/// Reference to a control having both a mute and gain setting
pub struct Gate {
    /// Address controlling whether audio is enabled, 1 = off 2 = on
    pub enable: u16,

    /// Address where the gain is controlled
    pub gain: u16,
}
