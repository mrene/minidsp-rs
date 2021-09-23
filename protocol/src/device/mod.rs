//! Static device definitions
//!
//! This is where support for other devices should be added

#[allow(unused_imports)]
use super::Source::{self as Source, *};

mod probe;
pub use probe::{by_kind, probe, probe_kind, DeviceKind};

#[cfg(feature = "device_2x4hd")]
pub mod m2x4hd;

#[cfg(feature = "device_4x10hd")]
pub mod m4x10hd;

#[cfg(feature = "device_msharc4x8")]
pub mod msharc4x8;

#[cfg(feature = "device_shd")]
pub mod shd;

#[cfg(feature = "device_ddrc24")]
pub mod ddrc24;

#[cfg(feature = "device_ddrc88bm")]
pub mod ddrc88bm;

#[cfg(feature = "device_nanodigi2x8")]
pub mod nanodigi2x8;

pub static GENERIC: Device = Device {
    product_name: "Generic",
    sources: &[],
    inputs: &[],
    outputs: &[],
    fir_max_taps: 0,
    internal_sampling_rate: 0,
    delay_mode: DelayMode::TenNanoseconds,
    #[cfg(feature = "symbols")]
    symbols: &[],
};

/// Defines how the high level api should interact with the device based on its memory layout
#[cfg_attr(feature = "debug", derive(Debug))]
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
    /// Whether the device accepts delay values in samples or in 0.01ms increments
    pub delay_mode: DelayMode,
    // A mapping of all symbols by name, as defined in the xml config
    #[cfg(feature = "symbols")]
    pub symbols: &'static [(&'static str, u16)],
}

/// Defines an input channel and its features
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Input {
    /// Mute and Gain
    pub gate: Option<Gate>,
    /// Volume Meter
    pub meter: Option<u16>,
    /// Parametric Equalizers
    pub peq: &'static [u16],
    /// Routing matrix, one entry per output channel connected to this input
    pub routing: &'static [Gate],
}

/// Defines an output channel and its features
#[cfg_attr(feature = "debug", derive(Debug))]
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
    pub xover: Option<Crossover>,
    /// Compressor
    pub compressor: Option<Compressor>,
    /// Address of the FIR bypass toggle
    pub fir: Option<Fir>,
}

/// Reference to a control having both a mute and gain setting
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Gate {
    /// Address controlling whether audio is enabled, 1 = off 2 = on
    pub enable: u16,

    /// Address where the gain is controlled
    pub gain: u16,
}
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Compressor {
    pub bypass: u16,
    pub threshold: u16,
    pub ratio: u16,
    pub attack: u16,
    pub release: u16,
    pub meter: Option<u16>,
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Crossover {
    /// First address of each biquad groups, each containing 4 sequential biquads.
    pub peqs: &'static [u16],
}

#[cfg_attr(feature = "debug", derive(Debug))]
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

#[cfg_attr(feature = "debug", derive(Debug))]
pub enum DelayMode {
    /// Delay values are in 0.01ms increments
    TenNanoseconds,

    /// Delay values are in samples, based on the device's internal sampling rate
    Samples,
}

impl Default for DelayMode {
    fn default() -> Self {
        DelayMode::TenNanoseconds
    }
}
