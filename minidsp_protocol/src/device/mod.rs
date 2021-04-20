//! Static device definitions
//!
//! This is where support for other devices should be added
//!
use super::Source::{self as Source, *};

mod probe;
pub use probe::probe;

pub mod m2x4hd;

/// Defines how the high level api should interact with the device based on its memory layout
#[cfg_attr(feature="debug", derive(Debug))]
pub struct Device {
    /// The name identifying the product, e.g. "2x4HD"
    pub product_name: &'static str,
    /// The name of the input sources
    pub sources: &'static [Source],
    /// The definitions for all input channels
    pub inputs: &'static [Input],
    /// The definitions for all output channels
    pub outputs: &'static [Output],
    /// Maximum total number of FIR taps
    pub fir_max_taps: u16,
    /// Internal sampling rate in Hz
    pub internal_sampling_rate: u32,
}

/// Defines an input channel and its features
#[cfg_attr(feature="debug", derive(Debug))]
pub struct Input {
    /// Mute and Gain
    pub gate: Gate,
    /// Volume Meter
    pub meter: u16,
    /// Parametric Equalizers
    pub peq: &'static [u16],
    /// Routing matrix, one entry per output channel connected to this input
    pub routing: &'static [Gate],
}

/// Defines an output channel and its features
#[cfg_attr(feature="debug", derive(Debug))]
pub struct Output {
    /// Mute and Gain
    pub gate: Gate,
    /// Volume Meter
    pub meter: u16,
    /// Address of the delay value
    pub delay_addr: u16,
    /// Address of the invert toggle
    pub invert_addr: u16,
    /// Parametric equalizers
    pub peq: &'static [u16],
    /// Crossover biquads
    pub xover: Crossover,
    /// Compressor
    pub compressor: Compressor,
    /// Address of the FIR bypass toggle
    pub fir: Fir,
}

/// Reference to a control having both a mute and gain setting
#[cfg_attr(feature="debug", derive(Debug))]
pub struct Gate {
    /// Address controlling whether audio is enabled, 1 = off 2 = on
    pub enable: u16,

    /// Address where the gain is controlled
    pub gain: u16,
}
#[cfg_attr(feature="debug", derive(Debug))]
pub struct Compressor {
    pub bypass: u16,
    pub threshold: u16,
    pub ratio: u16,
    pub attack: u16,
    pub release: u16,
    pub meter: u16,
}

#[cfg_attr(feature="debug", derive(Debug))]
pub struct Crossover {
    /// First address of each biquad groups, each containing 4 sequential biquads.
    pub peqs: &'static [u16],
}

#[cfg_attr(feature="debug", derive(Debug))]
pub struct Fir {
    /// Index to use in the FIRLoad commands
    pub index: u8,

    /// Address saving the number of active coefficients
    pub num_coefficients: u16,

    /// Bypass address
    pub bypass: u16,

    /// Maximum supported coefficients
    pub max_coefficients: u16,
}
