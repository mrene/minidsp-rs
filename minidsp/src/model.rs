///! Remote control object model
/// Exposes configurable components in a (de)serializable format, suitable
/// for various RPC protocols. Each field is optional, and will trigger an action if set.
use std::{fmt, time::Duration};

use anyhow::anyhow;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Biquad, BiquadFilter, Channel, Gain, MiniDSP, MiniDSPError, Source};

#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusSummary {
    pub master: MasterStatus,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub input_levels: Vec<f32>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub output_levels: Vec<f32>,
}

impl StatusSummary {
    pub async fn fetch(dsp: &MiniDSP<'_>) -> Result<Self, MiniDSPError> {
        let master = dsp.get_master_status().await?;
        let (input_levels, output_levels) = dsp.get_input_output_levels().await?;

        Ok(StatusSummary {
            master: master.into(),
            input_levels,
            output_levels,
        })
    }
}

impl fmt::Display for StatusSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{:?}", self.master)?;
        let strs: Vec<String> = self
            .input_levels
            .iter()
            .map(|x| format!("{:.1}", *x))
            .collect();
        writeln!(f, "Input levels: {}", strs.join(", "))?;

        let strs: Vec<String> = self
            .output_levels
            .iter()
            .map(|x| format!("{:.1}", *x))
            .collect();
        writeln!(f, "Output levels: {}", strs.join(", "))?;

        Ok(())
    }
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
/// Settings applying to all outputs
pub struct MasterStatus {
    /// Active configuration preset
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<u8>,

    /// Active source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,

    /// Volume in dB [-127, 0]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<Gain>,

    /// Mute status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,

    /// Dirac Live status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirac: Option<bool>,
}

impl fmt::Debug for MasterStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Maintain structure format since ezbeq depends on verbatim stdout output (hasn't migrated to -o json yet)
        f.debug_struct("MasterStatus")
            .field("preset", &self.preset.unwrap_or_default())
            .field("source", &self.source.unwrap_or(Source::NotInstalled))
            .field("volume", &self.volume.unwrap_or_default())
            .field("mute", &self.mute.unwrap_or_default())
            .field("dirac", &self.dirac.unwrap_or_default())
            .finish()
    }
}

impl From<minidsp_protocol::MasterStatus> for MasterStatus {
    fn from(s: minidsp_protocol::MasterStatus) -> Self {
        Self {
            preset: s.preset,
            source: s.source,
            volume: s.volume,
            mute: s.mute,
            dirac: s.dirac,
        }
    }
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

        if let Some(value) = self.dirac {
            dsp.set_dirac(value).await?;
        }

        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
/// Top-level configuration object that can be applied to a device
pub struct Config {
    /// Global settings not affected by config presets
    pub master_status: Option<MasterStatus>,

    /// Input channels, only the relevant inputs need to be included
    pub inputs: Vec<Input>,

    /// Output channels, only the relevant outputs need to be included
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

        for output in &self.outputs {
            let output_index = output.index.ok_or_else(|| {
                MiniDSPError::InternalError(anyhow!("missing output index field"))
            })?;

            output.apply(&dsp.output(output_index)?).await?;
        }

        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Gate {
    // If set, controls whether this channel is muted
    pub mute: Option<bool>,

    // If set, sets the channel gain
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
    /// The 0-based index of this input (required)
    pub index: Option<usize>,
    #[serde(flatten)]
    pub gate: Gate,

    /// Parametric equalizers
    pub peq: Vec<Peq>,

    /// Routing matrix
    pub routing: Vec<RoutingEntry>,
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

        for routing_entry in &self.routing {
            let index = routing_entry.index.ok_or_else(|| {
                MiniDSPError::InternalError(anyhow!(
                    "missing output channel index inside routing entry"
                ))
            })?;

            if let Some(gain) = routing_entry.gate.gain {
                input.set_output_gain(index, gain).await?;
            }

            if let Some(mute) = routing_entry.gate.mute {
                input.set_output_enable(index, !mute).await?;
            }
        }

        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct RoutingEntry {
    /// The 0-based index of the output channel (required)
    pub index: Option<usize>,

    #[serde(flatten)]
    pub gate: Gate,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Output {
    /// The 0-based index of this input (required)
    pub index: Option<usize>,

    #[serde(flatten)]
    pub gate: Gate,

    /// Parametric equalizers
    pub peq: Vec<Peq>,

    /// Phase inversion
    pub invert: Option<bool>,

    /// Time delay
    pub delay: Option<Duration>,

    /// Crossover (cascading biquads)
    pub crossover: Vec<Crossover>,

    /// Compressor settings
    pub compressor: Option<Compressor>,

    /// Finite Impulse Response filter
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
            let device_xover = output.crossover().ok_or(MiniDSPError::NoSuchPeripheral)?;
            xover.apply(&device_xover).await?;
        }

        if let Some(compressor) = &self.compressor {
            let device_compressor = output.compressor().ok_or(MiniDSPError::NoSuchPeripheral)?;
            compressor.apply(&device_compressor).await?;
        }

        if let Some(fir) = &self.fir {
            let device_fir = output.fir().ok_or(MiniDSPError::NoSuchPeripheral)?;
            fir.apply(&device_fir).await?;
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
    /// If set, bypasses the FIR filter
    pub bypass: Option<bool>,

    /// Filter coefficients
    /// Automatically sets the number of active taps and re-enables audio after setting the filter's coefficients.
    /// An empty array will clear the filter and reset its coefficients.
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
