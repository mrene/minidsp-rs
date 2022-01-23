use bimap::BiMap;
use minidsp_protocol::device::Device;

#[derive(Clone, Copy, Debug)]
pub enum Type {
    Int(isize, isize),
    Decibel(f32),
    Float32(f32, f32),
    Source(Source),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    Source,
    Input(usize, ChannelComponent),
    Output(usize, ChannelComponent),
    RoutingMatrix(usize, usize, GateComponent),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelComponent {
    Gate(GateComponent),
    Meter,
    PEQ(usize),
    Delay,
    Invert,
    Crossover(usize),
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
pub enum Source {
    NotInstalled,
    Analog,
    Toslink,
    Spdif,
    Usb,
    Aesebu,
    Rca,
    Xlr,
    Lan,
    I2S,
}

pub struct Resolver {
    device: &'static Device,
}

impl Resolver {
    pub fn addr_map(&self) -> BiMap<Target, u16> {
        let mut map = BiMap::new();
        let mut add = |target: Target| {
            if let Some(o) = self.resolve(target) {
                map.insert(target, o);
            }
        };

        for (i, input) in self.device.inputs.iter().enumerate() {
            add(Target::Input(i, ChannelComponent::Gate(GateComponent::Enable)));
            add(Target::Input(i, ChannelComponent::Gate(GateComponent::Gain)));

            add(Target::Input(i, ChannelComponent::Meter));

            for (j, _) in input.peq.iter().enumerate() {
                add(Target::Input(i, ChannelComponent::PEQ(j)));
            }

            for (j, _) in self.device.outputs.iter().enumerate() {
                add(Target::RoutingMatrix(i, j, GateComponent::Gain));
                add(Target::RoutingMatrix(i, j, GateComponent::Enable));
            }
        }
        
        for (i, output) in self.device.outputs.iter().enumerate() {
            add(Target::Output(i, ChannelComponent::Gate(GateComponent::Enable)));
            add(Target::Output(i, ChannelComponent::Gate(GateComponent::Gain)));

            add(Target::Output(i, ChannelComponent::Meter));
            add(Target::Output(i, ChannelComponent::Delay));
            add(Target::Output(i, ChannelComponent::Invert));

            for (j, _) in output.peq.iter().enumerate() {
                add(Target::Output(i, ChannelComponent::PEQ(j)));
            }

            if let Some(xover) = output.xover.as_ref() {
                for (j, _) in xover.peqs.iter().enumerate() {
                    add(Target::Output(i, ChannelComponent::Crossover(j)));
                }
            }

            if output.compressor.is_some() {
                add(Target::Output(i, ChannelComponent::Compressor(CompressorComponent::Bypass)));
                add(Target::Output(i, ChannelComponent::Compressor(CompressorComponent::Meter)));
                add(Target::Output(i, ChannelComponent::Compressor(CompressorComponent::Threshold)));
                add(Target::Output(i, ChannelComponent::Compressor(CompressorComponent::Ratio)));
                add(Target::Output(i, ChannelComponent::Compressor(CompressorComponent::Attack)));
                add(Target::Output(i, ChannelComponent::Compressor(CompressorComponent::Release)));
            }

            if output.fir.is_some() {
                add(Target::Output(i, ChannelComponent::Fir(FIRComponent::Bypass)));
                add(Target::Output(i, ChannelComponent::Fir(FIRComponent::NumCoefficients)));
            }
        }
        
        map
    }

    pub fn resolve(&self, target: Target) -> Option<u16> {
        use Target::*;

        Some(match target {
            Source => None?,
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
                    ChannelComponent::PEQ(peq_index) => *input.peq.get(peq_index)?,
                    _ => None?,
                }
            }
            Output(output, comp) => {
                let output = self.device.outputs.get(output)?;
                match comp {
                    ChannelComponent::Gate(gatecomp) => {
                        match gatecomp {
                            GateComponent::Enable => output.gate.enable,
                            GateComponent::Gain => output.gate.gain?,
                        }
                    },
                    ChannelComponent::Meter => output.meter?,
                    ChannelComponent::PEQ(peq_index) => *output.peq.get(peq_index)?,
                    ChannelComponent::Delay => output.delay_addr?,
                    ChannelComponent::Invert => output.invert_addr,
                    // TODO: This only references the first biquad, other xover addresses need to be visible
                    ChannelComponent::Crossover(xover_index) => *output.xover.as_ref()?.peqs.get(xover_index)?,
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
                    },
                    ChannelComponent::Fir(comp) => {
                        let fir = output.fir.as_ref()?;
                        match comp {
                            FIRComponent::Bypass => fir.bypass,
                            FIRComponent::Cofficients => todo!(),
                            FIRComponent::NumCoefficients => fir.num_coefficients,
                        }
                    }
                }
            },
            RoutingMatrix(input, output, comp) => {
                let gate = self.device.inputs.get(input)?.routing.get(output)?;
                match comp {
                    GateComponent::Enable => gate.enable,
                    GateComponent::Gain => gate.gain?,
                }
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

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
        let map = resolver.addr_map();
        let mut pairs: Vec<_> = map.iter().map(|(x,y)| (*x, *y)).collect();
        pairs.sort_by_key(|x| x.0);
        
        println!("{:#?}", pairs);
    }}
