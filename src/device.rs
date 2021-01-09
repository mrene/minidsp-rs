//! Static device definitions
//!
//! This is where support for other devices should be added
//!

use super::Source::{self as Source, *};

pub const DEVICE_2X4HD: Device = Device {
    sources: &[Analog, Toslink, Usb],
    inputs: &[
        Input {
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

/// Defines how the high level api should interact with the device based on its memory layout
pub struct Device {
    /// The name of the input sources
    pub sources: &'static [Source],
    /// The definitions for all input channels
    pub inputs: &'static [Input],
    /// The definitions for all output channels
    pub outputs: &'static [Output],
}

/// Defines an input channel and its features
pub struct Input {
    /// Mute and Gain
    pub gate: Gate,
    /// Parametric Equalizers
    pub peq: PEQ,
    /// Routing matrix, one entry per output channel connected to this input
    pub routing: &'static [Gate],
}

/// Defines an output channel and its features
pub struct Output {
    /// Mute and Gain
    pub gate: Gate,
    /// Address of the delay value
    pub delay_addr: u16,
    /// Address of the invert toggle
    pub invert_addr: u16,
    /// Parametric equalizers
    pub peq: PEQ,
    // TODO: Xover
    // TODO: Compressor
    // pub fir_coeff_addr: u16,
    // XXX: TODO: active=2 bypass=3 via 0x13 0x80
    /// Address of the FIR bypass toggle
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
    /// For the 2x4HD, this would span from `.at(0)` to `.at(9)`
    pub fn at(&self, index: usize) -> u16 {
        if index >= self.len {
            panic!("out of bounds peq access index={} len={}", index, self.len);
        }
        self.high - (index * 5) as u16
    }

    pub fn iter(&'_ self) -> impl '_ + Iterator<Item = u16> {
        (0..self.len).map(move |x| self.at(x))
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
