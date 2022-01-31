use bimap::BiMap;
use minidsp_protocol::{
    commands::{Addr, Gain},
    device::Device,
    Commands, DeviceInfo, Source,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Mismatched types. expected={expected:?} actual={actual:?}")]
    MismatchedTypes {
        expected: TypeKind,
        actual: TypeKind,
    },
    #[error("No such peripheral exists")]
    NoSuchPeripheral,
}

#[derive(Clone, Debug)]
pub enum Value {
    // Integer
    Int(u8),
    // Float value representing a decibel quantity
    Decibel(f32),
    // Verbatim float value
    Float32(f32),
    // Input source
    Source(Source),
    // Boolean value
    Bool(bool),
    // Biquad coefficients
    Biquad([f32; 5]),
    // FIR coefficients of various length
    Coefficients(Vec<f32>),
}

impl Value {
    pub fn as_source(self) -> Result<Source, Error> {
        Ok(match self {
            Value::Source(src) => src,
            _ => Err(Error::MismatchedTypes {
                expected: TypeKind::Source,
                actual: self.kind(),
            })?,
        })
    }

    pub fn as_decibel(self) -> Result<f32, Error> {
        Ok(match self {
            Value::Decibel(db) => db,
            _ => Err(Error::MismatchedTypes {
                expected: TypeKind::Decibel,
                actual: self.kind(),
            })?,
        })
    }

    fn as_bool(&self) -> Result<bool, Error> {
        Ok(match self {
            &Value::Bool(val) => val,
            _ => Err(Error::MismatchedTypes {
                expected: TypeKind::Bool,
                actual: self.kind(),
            })?,
        })
    }

    fn as_int(&self) -> Result<u8, Error> {
        Ok(match self {
            &Value::Int(val) => val,
            _ => Err(Error::MismatchedTypes {
                expected: TypeKind::Int,
                actual: self.kind(),
            })?,
        })
    }

    fn as_biquad(&self) -> Result<[f32; 5], Error> {
        Ok(match self {
            &Value::Biquad(val) => val,
            _ => Err(Error::MismatchedTypes {
                expected: TypeKind::Biquad,
                actual: self.kind(),
            })?,
        })
    }

    pub fn kind(&self) -> TypeKind {
        match self {
            Value::Int(_) => TypeKind::Int,
            Value::Decibel(_) => TypeKind::Decibel,
            Value::Float32(_) => TypeKind::Float32,
            Value::Source(_) => TypeKind::Source,
            Value::Bool(_) => TypeKind::Bool,
            Value::Biquad(_) => TypeKind::Biquad,
            Value::Coefficients(_) => TypeKind::Coefficients,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TypeKind {
    Int,
    // Input source
    Source,
    // Boolean
    Bool,
    // Decibel value (will be converted as needed)
    Decibel,
    // Verbatim float value
    Float32,
    // Biquad coefficients
    Biquad,
    // FIR coefficients
    Coefficients,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    Source,
    Gain,
    Mute,
    Config,
    Dirac,
    Input(usize, ChannelComponent),
    Output(usize, ChannelComponent),
    RoutingMatrix(usize, usize, GateComponent),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelComponent {
    Gate(GateComponent),
    Meter,
    PEQ(usize, PEQComponent),
    Delay,
    Invert,
    Crossover(usize, u16),
    Compressor(CompressorComponent),
    Fir(FIRComponent),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum GateComponent {
    Enable,
    Gain,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum FIRComponent {
    Bypass,
    Cofficients,
    NumCoefficients,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompressorComponent {
    Bypass,
    Threshold,
    Ratio,
    Attack,
    Release,
    Meter,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum PEQComponent {
    Bypass,
    Coefficients,
}

// #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
// pub enum Source {
//     NotInstalled,
//     Analog,
//     Toslink,
//     Spdif,
//     Usb,
//     Aesebu,
//     Rca,
//     Xlr,
//     Lan,
//     I2S,
// }

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandType {
    Write,
    WriteBiquad,
    WriteBiquadBypass,
}

#[derive(Clone, Debug)]
pub enum Operation {
    Read(Target),
    Write(Target, Value),
}

pub struct Resolver {
    device: &'static Device,
}

impl Resolver {
    /// Build a BiMap for addresses used with input and output channels
    pub fn channels_addr_map(&self) -> BiMap<Target, u16> {
        let mut map = BiMap::new();
        let mut add = |target: Target| {
            if let Some(o) = self.resolve_addr(target) {
                map.insert(target, o);
            }
        };

        for (i, input) in self.device.inputs.iter().enumerate() {
            add(Target::Input(
                i,
                ChannelComponent::Gate(GateComponent::Enable),
            ));
            add(Target::Input(
                i,
                ChannelComponent::Gate(GateComponent::Gain),
            ));

            add(Target::Input(i, ChannelComponent::Meter));

            for (j, _) in input.peq.iter().enumerate() {
                add(Target::Input(
                    i,
                    ChannelComponent::PEQ(j, PEQComponent::Bypass),
                ));
            }

            for (j, _) in self.device.outputs.iter().enumerate() {
                add(Target::RoutingMatrix(i, j, GateComponent::Gain));
                add(Target::RoutingMatrix(i, j, GateComponent::Enable));
            }
        }

        for (i, output) in self.device.outputs.iter().enumerate() {
            add(Target::Output(
                i,
                ChannelComponent::Gate(GateComponent::Enable),
            ));
            add(Target::Output(
                i,
                ChannelComponent::Gate(GateComponent::Gain),
            ));

            add(Target::Output(i, ChannelComponent::Meter));
            add(Target::Output(i, ChannelComponent::Delay));
            add(Target::Output(i, ChannelComponent::Invert));

            for (j, _) in output.peq.iter().enumerate() {
                add(Target::Output(
                    i,
                    ChannelComponent::PEQ(j, PEQComponent::Coefficients),
                ));
            }

            if let Some(xover) = output.xover.as_ref() {
                for (j, _) in xover.peqs.iter().enumerate() {
                    // FIXME: Assuming each xover group contains 4 biquads
                    for k in 0..4 {
                        add(Target::Output(i, ChannelComponent::Crossover(j, k)));
                    }
                }
            }

            if output.compressor.is_some() {
                add(Target::Output(
                    i,
                    ChannelComponent::Compressor(CompressorComponent::Bypass),
                ));
                add(Target::Output(
                    i,
                    ChannelComponent::Compressor(CompressorComponent::Meter),
                ));
                add(Target::Output(
                    i,
                    ChannelComponent::Compressor(CompressorComponent::Threshold),
                ));
                add(Target::Output(
                    i,
                    ChannelComponent::Compressor(CompressorComponent::Ratio),
                ));
                add(Target::Output(
                    i,
                    ChannelComponent::Compressor(CompressorComponent::Attack),
                ));
                add(Target::Output(
                    i,
                    ChannelComponent::Compressor(CompressorComponent::Release),
                ));
            }

            if output.fir.is_some() {
                add(Target::Output(
                    i,
                    ChannelComponent::Fir(FIRComponent::Bypass),
                ));
                add(Target::Output(
                    i,
                    ChannelComponent::Fir(FIRComponent::NumCoefficients),
                ));
            }
        }

        map
    }

    pub fn resolve_addr(&self, target: Target) -> Option<u16> {
        use Target::*;

        Some(match target {
            Input(input, comp) => {
                let input = self.device.inputs.get(input)?;
                match comp {
                    ChannelComponent::Gate(gatecomp) => {
                        let gate = input.gate.as_ref()?;
                        match gatecomp {
                            GateComponent::Enable => gate.enable,
                            GateComponent::Gain => gate.gain?,
                        }
                    }
                    ChannelComponent::Meter => input.meter?,
                    ChannelComponent::PEQ(peq_index, PEQComponent::Bypass) => {
                        *input.peq.get(peq_index)?
                    }
                    _ => None?,
                }
            }
            Output(output, comp) => {
                let output = self.device.outputs.get(output)?;
                match comp {
                    ChannelComponent::Gate(gatecomp) => match gatecomp {
                        GateComponent::Enable => output.gate.enable,
                        GateComponent::Gain => output.gate.gain?,
                    },
                    ChannelComponent::Meter => output.meter?,
                    ChannelComponent::PEQ(peq_index, _) => *output.peq.get(peq_index)?,
                    ChannelComponent::Delay => output.delay_addr?,
                    ChannelComponent::Invert => output.invert_addr,
                    // TODO: This only references the first biquad, other xover addresses need to be visible
                    ChannelComponent::Crossover(xover_index, index) => {
                        *output.xover.as_ref()?.peqs.get(xover_index)? + index
                    }
                    ChannelComponent::Compressor(comp) => {
                        let compressor = output.compressor.as_ref()?;
                        match comp {
                            CompressorComponent::Bypass => compressor.bypass,
                            CompressorComponent::Threshold => compressor.threshold,
                            CompressorComponent::Ratio => compressor.ratio,
                            CompressorComponent::Attack => compressor.attack,
                            CompressorComponent::Release => compressor.release,
                            CompressorComponent::Meter => compressor.meter?,
                        }
                    }
                    ChannelComponent::Fir(comp) => {
                        let fir = output.fir.as_ref()?;
                        match comp {
                            FIRComponent::Bypass => fir.bypass,
                            FIRComponent::Cofficients => todo!(),
                            FIRComponent::NumCoefficients => fir.num_coefficients,
                        }
                    }
                }
            }
            RoutingMatrix(input, output, comp) => {
                let gate = self.device.inputs.get(input)?.routing.get(output)?;
                match comp {
                    GateComponent::Enable => gate.enable,
                    GateComponent::Gain => gate.gain?,
                }
            }
            _ => None?,
        })
    }
}

pub struct Dialect {
    pub device_info: DeviceInfo,
    pub resolver: Resolver,
    pub dialect: minidsp_protocol::Dialect,
}
impl Dialect {
    pub fn encode_op(&self, op: Operation) -> Result<Commands, Error> {
        Ok(match op {
            Operation::Write(target, value) => match target {
                Target::Config => Commands::SetConfig {
                    config: value.as_int()?,
                    reset: true,
                },
                Target::Source => Commands::SetSource {
                    source: value.as_source()?.to_id(&self.device_info),
                },
                Target::Gain => Commands::SetVolume {
                    value: Gain(value.as_decibel()?),
                },
                Target::Mute => Commands::SetMute {
                    value: value.as_bool()?,
                },
                Target::Dirac => Commands::DiracBypass {
                    value: if value.as_bool()? { 0 } else { 1 },
                },
                Target::Input(_, component) => {
                    let addr = self.resolver.resolve_addr(target);
                    let addr = addr
                        .map(|addr| self.dialect.addr(addr))
                        .ok_or(Error::NoSuchPeripheral);
                    match component {
                        ChannelComponent::PEQ(_, peq_comp) => match peq_comp {
                            PEQComponent::Bypass => Commands::WriteBiquadBypass {
                                addr: addr?,
                                value: value.as_bool()?,
                            },
                            PEQComponent::Coefficients => Commands::WriteBiquad {
                                addr: addr?,
                                data: value.as_biquad()?.map(|coeff| self.dialect.float(coeff)),
                            },
                        },
                        ChannelComponent::Gate(_) => todo!(),
                        ChannelComponent::Meter => todo!(),
                        ChannelComponent::Delay => todo!(),
                        ChannelComponent::Invert => todo!(),
                        ChannelComponent::Crossover(_, _) => todo!(),
                        ChannelComponent::Compressor(_) => todo!(),
                        ChannelComponent::Fir(_) => todo!(),
                    }
                }
                Target::Output(_, _) => todo!(),
                Target::RoutingMatrix(_, _, _) => todo!(),
            },
            Operation::Read(_) => todo!(),
        })
    }
}

pub struct Driver;

impl Driver {
    pub fn execute(&self, op: &Operation) -> Option<Value> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bimap::BiMap;
    use minidsp_protocol::device::m2x4hd::DEVICE;

    fn add_sym(h: &mut BiMap<Target, u16>, t: Target, f: impl Fn() -> Option<u16>) {
        if let Some(sym) = f() {
            h.insert(t, sym);
        }
    }

    #[test]
    fn it_works() {
        let resolver = Resolver { device: &DEVICE };
        let map = resolver.channels_addr_map();
        let mut pairs: Vec<_> = map.iter().map(|(x, y)| (*x, *y)).collect();
        pairs.sort_by_key(|x| x.0);

        println!("{:#?}", pairs);
    }
}
