use minidsp::device::DelayMode;

/// Defines how the high level api should interact with the device based on its memory layout
#[derive(Debug)]
pub struct Device {
    /// The name identifying the product, e.g. "2x4HD"
    pub product_name: String,
    /// The name of the input sources
    pub sources: Vec<String>,
    /// The definitions for all input channels
    pub inputs: Vec<Input>,
    /// The definitions for all output channels
    pub outputs: Vec<Output>,
    /// Maximum total number of FIR taps
    pub fir_max_taps: u16,
    /// Internal sampling rate in Hz
    pub internal_sampling_rate: u32,
    /// Whether the device accepts delay values in samples or in 0.01ms increments
    pub delay_mode: DelayMode,
}

/// Defines an input channel and its features
#[derive(Debug)]
pub struct Input {
    /// Mute and Gain
    pub gate: Option<Gate>,
    /// Volume Meter
    pub meter: Option<String>,
    /// Parametric Equalizers
    pub peq: Vec<String>,
    /// Routing matrix, one entry per output channel connected to this input
    pub routing: Vec<Gate>,
}

/// Defines an output channel and its features
#[derive(Debug)]
pub struct Output {
    /// Mute and Gain
    pub gate: Gate,
    /// Volume Meter
    pub meter: String,
    /// Address of the delay value
    pub delay_addr: String,
    /// Address of the invert toggle
    pub invert_addr: String,
    /// Parametric equalizers
    pub peq: Vec<String>,
    /// Crossover biquads
    pub xover: Option<Crossover>,
    /// Compressor
    pub compressor: Option<Compressor>,
    /// Address of the FIR bypass toggle
    pub fir: Option<Fir>,
}

/// Reference to a control having both a mute and gain setting
#[derive(Debug)]
pub struct Gate {
    /// Address controlling whether audio is enabled, 1 = off 2 = on
    pub enable: String,

    /// Address where the gain is controlled
    pub gain: String,
}
#[derive(Debug)]
pub struct Compressor {
    pub bypass: String,
    pub threshold: String,
    pub ratio: String,
    pub attack: String,
    pub release: String,
    pub meter: Option<String>,
}

#[derive(Debug)]
pub struct Crossover {
    /// First address of each biquad groups, each containing 4 sequential biquads.
    pub peqs: Vec<String>,
}

#[derive(Debug)]
pub struct Fir {
    /// Index to use in the FIRLoad commands
    pub index: u8,

    /// Address saving the number of active coefficients
    pub num_coefficients: String,

    /// Bypass address
    pub bypass: String,

    /// Maximum supported coefficients
    pub max_coefficients: u16,
}
