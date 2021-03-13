////! Remote control object model
/// Exposes configurable components in a (de)serializable format, suitable for various RPC protocols. Each field is optional, and will trigger an action if set.

use crate::{Biquad, BiquadFilter, Channel, Gain, MiniDSP, MiniDSPError, Source};
use anyhow::anyhow;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
/// Settings applying to all outputs
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

impl MasterStatus {
    pub async fn apply(&self, dsp: &MiniDSP<'_>) -> Result<(), MiniDSPError> {
        if let Some(config) = self.preset {
            dsp.set_config(config).await?;
        }

        if let Some(source) = self.source {
            dsp.set_source(source).await?;
        }

        if let Some(value) = self.volume {
            dsp.set_master_volume(value).await?;
        }

        if let Some(value) = self.mute {
            dsp.set_master_mute(value).await?;
        }

        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
/// Top-level configuration object that can be applied to a device
pub struct Config {
    pub master_status: Option<MasterStatus>,
    pub inputs: Vec<Input>,
}

impl Config {
    pub async fn apply(&self, dsp: &MiniDSP<'_>) -> Result<(), MiniDSPError> {
        // Always set master status first, since it might change the current active preset
        if let Some(master_status) = &self.master_status {
            master_status.apply(&dsp).await?;
        }

        for input in &self.inputs {
            let input_index = input.index.ok_or(MiniDSPError::InternalError(anyhow!(
                "missing input index field"
            )))?;
            input.apply(&dsp.input(input_index)?).await?;
        }

        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Gate {
    pub mute: Option<bool>,
    pub gain: Option<Gain>,
}

impl Gate {
    pub async fn apply<C: Channel + Send + Sync>(&self, channel: &C) -> Result<(), MiniDSPError> {
        if let Some(mute) = self.mute {
            channel.set_mute(mute).await?;
        }

        if let Some(gain) = self.gain {
            channel.set_gain(gain).await?;
        }

        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Input {
    pub index: Option<usize>,
    #[serde(flatten)]
    pub gate: Gate,
    pub peq: Vec<Peq>,
}

impl Input {
    pub async fn apply(&self, input: &crate::Input<'_>) -> Result<(), MiniDSPError> {
        self.gate.apply(input).await?;

        for peq in &self.peq {
            let peq_index = peq.index.ok_or(MiniDSPError::InternalError(anyhow!(
                "missing peq index field"
            )))?;

            peq.apply(&input.peq(peq_index)?).await?;
        }
        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Peq {
    pub index: Option<usize>,
    pub coeff: Option<Biquad>,
    pub bypass: Option<bool>,
}

impl Peq {
    pub async fn apply(&self, peq: &BiquadFilter<'_>) -> Result<(), MiniDSPError> {
        if let Some(bypass) = self.bypass {
            peq.set_bypass(bypass).await?;
        }

        if let Some(ref coeff) = self.coeff {
            peq.set_coefficients(&coeff.to_array()[..]).await?;
        }

        Ok(())
    }
}
