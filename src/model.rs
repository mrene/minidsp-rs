/*! Revised object model

The objective is to represent most/all of the config state into a single object, and to implement a trait to apply them
based on a device definition.

This will mostly mimic the device structure where all the addresses are identified.
Would there be a way to generate one from the other?
There still needs to be a way to "bind" a component to an instance by mapping the right addresses in place
*/

use anyhow::anyhow;
use crate::{Biquad, Gain, Source, client::Client, commands::{Commands, WriteInt}, device, transport::MiniDSPError};
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// The current settings applying to all outputs
pub struct MasterStatus {
    /// Active configuration preset
    pub preset: Option<u8>,

    /// Active source
    pub source: Option<Source>,

    /// Volume in dB [-127, 0]
    pub volume: Option<Gain>,

    /// Mute status
    pub mute: Option<bool>,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub inputs: Vec<Input>,
}

impl Config {
    async fn apply(&self, client: &Client, spec: &device::Device) -> Result<(), MiniDSPError> {
        for input in &self.inputs {
            let input_index = input.index.ok_or(MiniDSPError::InternalError(anyhow!(
                "missing input index field"
            )))?;
            if input_index >= spec.inputs.len() {
                return Err(MiniDSPError::InternalError(anyhow!(
                    "Input index out of range ({} >= {})",
                    input_index,
                    spec.inputs.len()
                )));
            }
            input.apply(client, &spec.inputs[input_index]).await?;
        }

        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Gate {
    pub enable: Option<bool>,
    pub gain: Option<Gain>,
}

impl Gate {
    async fn apply(&self, client: &Client, spec: &device::Gate) -> Result<(), MiniDSPError> {
        if let Some(enable) = self.enable {
            let value = if enable {
                WriteInt::ENABLED
            } else {
                WriteInt::DISABLED
            };
            client.write_dsp(spec.enable, value).await?;
        }

        if let Some(gain) = self.gain {
            client.write_dsp(spec.gain, gain.0).await?;
        }

        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Input {
    pub index: Option<usize>,
    #[serde(flatten)]
    pub gate: Gate,
    pub peq: Vec<Peq>,
}

impl Input {
    async fn apply(&self, client: &Client, spec: &device::Input) -> Result<(), MiniDSPError> {
        self.gate.apply(client, &spec.gate).await?;
        for peq in &self.peq {
            let peq_index = peq.index.ok_or(MiniDSPError::InternalError(anyhow!(
                "missing peq index field"
            )))?;

            if peq_index >= spec.peq.len() {
                return Err(MiniDSPError::InternalError(anyhow!(
                    "PEQ index out of range ({} >= {})",
                    peq_index,
                    spec.peq.len()
                )));
            }
            peq.apply(client, spec.peq[peq_index]).await?;
        }
        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Peq {
    pub index: Option<usize>,
    pub coeff: Option<Biquad>,
    pub bypass: Option<bool>,
}

impl Peq {
    async fn apply(&self, client: &Client, addr: u16) -> Result<(), MiniDSPError> {
        if let Some(bypass) = self.bypass {
            client
                .roundtrip(Commands::WriteBiquadBypass {
                    addr,
                    value: bypass,
                })
                .await?
                .into_ack()?;
        }

        if let Some(ref coeff) = self.coeff {
            client
                .roundtrip(Commands::WriteBiquad {
                    addr,
                    data: coeff.into(),
                })
                .await?
                .into_ack()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // use serde_json::to_string_pretty;

    #[test]
    fn foo() {
        let cfg = Config {
            inputs: vec![
                Input {
                    index: Some(0),
                    gate: Gate {
                        enable: Some(true),
                        gain: Some(Gain(-40.)),
                    },
                    peq: Vec::new(),
                },
                Input::default(),
            ],
        };

        let cfgs = serde_json::to_string_pretty(&cfg).unwrap();

        println!("{}", cfgs);

        let s = r#"
        {
            "inputs": [
                {
                    "index": 0,
                    "peq": [{"index": 0, "bypass": true}]
                }
            ]
        }
        "#;

        let wtf: Config = serde_json::from_str(s).unwrap();
        println!("{:#?}", &wtf);

        let b = serde_cbor::to_vec(&wtf).unwrap();
        println!("{}", hex::encode(&b));
    }
}
