use std::time::Duration;

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
    pub outputs: Vec<Output>,
}

impl Config {
    pub async fn apply(&self, dsp: &MiniDSP<'_>) -> Result<(), MiniDSPError> {
        // Always set master status first, since it might change the current active preset
        if let Some(master_status) = &self.master_status {
            master_status.apply(&dsp).await?;
        }

        for input in &self.inputs {
            let input_index = input
                .index
                .ok_or_else(|| MiniDSPError::InternalError(anyhow!("missing input index field")))?;
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
            let peq_index = peq
                .index
                .ok_or_else(|| MiniDSPError::InternalError(anyhow!("missing peq index field")))?;

            peq.apply(&input.peq(peq_index)?).await?;
        }
        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Output {
    #[serde(flatten)]
    pub gate: Gate,
    pub peq: Vec<Peq>,
    pub invert: Option<bool>,
    pub delay: Option<Duration>,
    pub crossover: Vec<Crossover>,
    pub compressor: Option<Compressor>,
    pub fir: Option<Fir>,
}

impl Output {
    pub async fn apply(&self, output: &crate::Output<'_>) -> Result<(), MiniDSPError> {
        self.gate.apply(output).await?;

        for peq in &self.peq {
            let peq_index = peq
                .index
                .ok_or_else(|| MiniDSPError::InternalError(anyhow!("missing peq index field")))?;

            peq.apply(&output.peq(peq_index)?).await?;
        }

        if let Some(invert) = self.invert {
            output.set_invert(invert).await?;
        }

        if let Some(delay) = self.delay {
            output.set_delay(delay).await?;
        }

        for xover in &self.crossover {
            xover.apply(&output.crossover()).await?;
        }

        if let Some(compressor) = &self.compressor {
            compressor.apply(&output.compressor()).await?;
        }

        if let Some(fir) = &self.fir {
            fir.apply(&output.fir()).await?;
        }

        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Crossover {
    // Crossover group index (most likely 0 or 1)
    pub index: Option<usize>,
    pub coeff: Vec<Biquad>,
    pub bypass: Option<bool>,
}

impl Crossover {
    pub async fn apply(&self, xover: &crate::Crossover<'_>) -> Result<(), MiniDSPError> {
        let group = self
            .index
            .ok_or_else(|| MiniDSPError::InternalError(anyhow!("Invalid crossover group index")))?;

        for c in &self.coeff {
            if c.index.is_none() {
                return Err(MiniDSPError::InternalError(anyhow!(
                    "biquad index not specified"
                )));
            }
        }

        for c in &self.coeff {
            xover
                .set_coefficients(group, c.index.unwrap() as usize, &c.to_array()[..])
                .await?;
        }

        if let Some(bypass) = self.bypass {
            xover.set_bypass(group, bypass).await?;
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

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Compressor {
    pub bypass: Option<bool>,
    pub threshold: Option<f32>,
    pub ratio: Option<f32>,
    pub attack: Option<f32>,
    pub release: Option<f32>,
}

impl Compressor {
    pub async fn apply(&self, compressor: &crate::Compressor<'_>) -> Result<(), MiniDSPError> {
        if let Some(bypass) = self.bypass {
            compressor.set_bypass(bypass).await?;
        }
        if let Some(threshold) = self.threshold {
            compressor.set_threshold(threshold).await?;
        }
        if let Some(ratio) = self.ratio {
            compressor.set_ratio(ratio).await?;
        }
        if let Some(attack) = self.attack {
            compressor.set_attack(attack).await?;
        }
        if let Some(release) = self.release {
            compressor.set_release(release).await?;
        }

        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Fir {
    pub bypass: Option<bool>,
    pub coefficients: Option<Vec<f32>>,
}

impl Fir {
    pub async fn apply(&self, fir: &crate::Fir<'_>) -> Result<(), MiniDSPError> {
        if let Some(bypass) = self.bypass {
            fir.set_bypass(bypass).await?;
        }
        if let Some(coefficients) = &self.coefficients {
            if coefficients.is_empty() {
                fir.clear().await?;
            } else {
                fir.set_coefficients(coefficients).await?;
            }
        }
        Ok(())
    }
}
