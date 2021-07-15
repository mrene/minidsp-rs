#![allow(dead_code)]
//! EEPROM Addresses

// u8 Firmware Version
pub const FIRMWARE_VERSION: u16 = 0xFFA1;

// u32 Timestamp
// Value that gets updated to a random value whenever a setting is changed.
pub const TIMESTAMP: u16 = 0xFFC8;

// u8 Preset
// Current input preset
pub const PRESET: u16 = 0xFFD8;

// u8 Source
// Current input source (also known as "Digital IO")
pub const SOURCE: u16 = 0xFFD9;

// u8 Source
// Only used for unsolicited update messages
pub const SOURCE_ASYNC: u16 = 0xFFA9;

// u8 Master Volume (also known as "Codec mute")
pub const MASTER_VOLUME: u16 = 0xFFDA;

// u8 Mute
pub const MUTE: u16 = 0xFFDB;

// u8 Dirac Live bypass (also known as: "Master FIR bypass")
pub const DIRAC_BYPASS: u16 = 0xFFE0;

// u8 Channel mode
pub const CHANNEL_MODE: u16 = 0xFFE5;

// u32 Serial (+900000) - also known as "board id" (32 bits version)
pub const SERIAL: u16 = 0xFFFC;

// u16 Serial (+900000) - also known as "board id" (16 bits version)
pub const SERIAL_SHORT: u16 = 0xFFFE;
