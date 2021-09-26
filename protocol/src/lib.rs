#![cfg_attr(not(feature = "std"), no_std)]

//! Main protocol implementation.
//!
//! This crate provides the basic components in order to be able to send and receive
//! commands and events from a device.
//!
//! It is meant to be as lean as possible in order to run in restricted environments.
//! For this reason, it doesn't include any transport implementations.

extern crate alloc;

mod util;

pub mod commands;
pub use commands::{Commands, FromMemory};

pub mod packet;
use device::{probe_kind, DeviceKind};
pub use packet::ParseError;

pub mod source;
pub use source::Source;

#[cfg(feature = "devices")]
pub mod device;

pub mod eeprom;

pub mod fixed_point;
pub use fixed_point::FixedPoint;

#[derive(Copy, Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
#[cfg_attr(
    feature = "use_serde",
    derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)
)]
/// Hardware id and dsp version
pub struct DeviceInfo {
    pub hw_id: u8,
    pub dsp_version: u8,
    pub serial: u32,
}

impl DeviceInfo {
    pub fn device_kind(&self) -> DeviceKind {
        probe_kind(self)
    }

    pub fn supports_dirac(&self) -> bool {
        matches!(self.dsp_version, 61 | 94 | 95 | 101 | 105)
    }
}

#[derive(Default, Clone, PartialEq)]
#[cfg_attr(feature = "debug", derive(Debug))]
#[cfg_attr(
    feature = "use_serde",
    derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)
)]
/// Settings applying to all outputs
pub struct MasterStatus {
    /// Active configuration preset
    pub preset: Option<u8>,

    /// Active source
    pub source: Option<Source>,

    /// Volume in dB [-127, 0]
    pub volume: Option<commands::Gain>,

    /// Mute status
    pub mute: Option<bool>,

    /// Dirac Live status
    pub dirac: Option<bool>,
}
