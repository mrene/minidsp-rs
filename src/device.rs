pub const DEVICE_2X4HD: Device = Device {
    source_names: &["Analog", "TOSLINK", "USB"],
    inputs: &[
        Input {
            index: 0,
            gate: Gate {
                enable: 0x00,
                gain: 0x1a,
            },
            peq: PEQ {
                high: 0x2085,
                len: 10,
            },
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
            peq: PEQ {
                high: 0x20b7,
                len: 10,
            },
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
    outputs: &[
        Output {
            index: 0,
            gate: Gate {
                enable: 0x02,
                gain: 0x1c,
            },
            delay_addr: 0x40,
            invert_addr: 0x50,
            peq: PEQ {
                high: 0x20e9,
                len: 10,
            },
            fir_bypass_addr: 0x0e,
        },
        Output {
            index: 1,
            gate: Gate {
                enable: 0x03,
                gain: 0x1d,
            },
            delay_addr: 0x41,
            invert_addr: 0x51,
            peq: PEQ {
                high: 0x211b,
                len: 10,
            },
            fir_bypass_addr: 0x0f,
        },
        Output {
            index: 2,
            gate: Gate {
                enable: 0x04,
                gain: 0x1e,
            },
            delay_addr: 0x42,
            invert_addr: 0x52,
            peq: PEQ {
                high: 0x214d,
                len: 10,
            },
            fir_bypass_addr: 0x10,
        },
        Output {
            index: 3,
            gate: Gate {
                enable: 0x5,
                gain: 0x1f,
            },
            delay_addr: 0x43,
            invert_addr: 0x53,
            peq: PEQ {
                high: 0x217f,
                len: 10,
            },
            fir_bypass_addr: 0x11,
        },
    ],
};

pub struct Device {
    pub source_names: &'static [&'static str],
    pub inputs: &'static [Input],
    pub outputs: &'static [Output],
}

pub struct Input {
    pub index: usize,
    pub gate: Gate,
    pub peq: PEQ,
    pub routing: &'static [Gate],
}

pub struct Output {
    pub index: usize,
    pub gate: Gate,
    pub delay_addr: u16,
    pub invert_addr: u16,
    pub peq: PEQ,
    // TODO: Xover
    // TODO: Compressor
    // pub fir_coeff_addr: u16,
    /// XXX: TODO: active=2 bypass=3 via 0x13 0x80
    pub fir_bypass_addr: u16,
}

/// Reference to a control having both a mute and gain setting
pub struct Gate {
    /// Address controlling whether audio is enabled, 1 = off 2 = on
    pub enable: u16,

    /// Address where the gain is controlled
    pub gain: u16,
}

/// A range of biquad filter address part of a single parametric eq
pub struct PEQ {
    /// Higher bound address
    pub high: u16,

    /// Number of filters available
    pub len: usize,
}

impl PEQ {
    /// Get the address for a specific filter
    /// To be compatible with the app's ordering, the first filter
    /// is the highest address while the last filter is the lowest one.
    pub fn at(&self, index: usize) -> u16 {
        if index >= self.len {
            panic!("out of bounds peq access index={} len={}", index, self.len);
        }
        self.high - (index * 5) as u16
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_peq() {
        let addrs = &[0xb7, 0xb2, 0xad, 0xa8, 0xa3, 0x9e, 0x99, 0x94, 0x8f, 0x8a];
        let peq = PEQ {
            high: addrs[0],
            len: 10,
        };
        let peq_addrs: Vec<_> = (0..10).map(|x| peq.at(x)).collect();
        assert!(peq_addrs.into_iter().eq(addrs.iter().cloned()));
    }
}
