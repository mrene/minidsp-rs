#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod commands;
pub mod packet;
pub mod source;
pub use source::Source;
pub mod device;

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
