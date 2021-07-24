//! Commands sent to the device and their responses
//!
//! This module contains structs that can be serialized into commands being sent to the device.
//! Each command implements the `UnaryCommand` trait which specifies the response type as an
//! associated type.
//!
//! It's typical to use the [roundtrip] method in order to send the command to a transport and
//! obtained its parsed response.

use alloc::vec::Vec;
use core::{convert::TryInto, fmt, fmt::Debug, ops::Deref, str::FromStr};
use std::ops::DerefMut;

use anyhow::Result;
use bytes::{Buf, BufMut, Bytes, BytesMut};
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "debug")]
use thiserror::Error;

use super::DeviceInfo;
use crate::{eeprom, packet::ParseError, util::TryBuf, util::TryBufError, MasterStatus};

/// Maximum number of floats that can be read in a single command
pub const READ_FLOATS_MAX: usize = 14;

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug, Error))]
pub enum ProtocolError {
    #[cfg_attr(feature = "debug", error("bad cmd id"))]
    BadCommandId,

    #[cfg_attr(feature = "debug", error("unexpected response type"))]
    UnexpectedResponseType,

    #[cfg_attr(feature = "debug", error("parse error: {0}"))]
    ParseError(ParseError),

    #[cfg_attr(
        feature = "debug",
        error("the received hardware id was malformed or empty")
    )]
    MalformedHardwareId,

    #[cfg_attr(feature = "debug", error("parse error: {0}"))]
    DecodeError(#[from] TryBufError),
}

#[derive(Clone)]
pub struct BytesWrap(pub Bytes);
impl fmt::Debug for BytesWrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.0.as_ref(), f)
    }
}
impl Deref for BytesWrap {
    type Target = Bytes;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for BytesWrap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone)]
pub enum Value {
    Unknown(Bytes),
    Float(f32),
    Int(u16),
}

impl From<f32> for Value {
    fn from(f: f32) -> Self {
        Value::Float(f)
    }
}

impl From<u16> for Value {
    fn from(x: u16) -> Self {
        Value::Int(x)
    }
}

impl Value {
    pub fn into_bytes(self) -> Bytes {
        match self {
            Value::Unknown(b) => b,
            Value::Float(f) => Bytes::copy_from_slice(&f.to_le_bytes()),
            Value::Int(i) => {
                let mut b = BytesMut::with_capacity(4);
                b.put_u16_le(i);
                b.put_u16(0x00);
                b.freeze()
            }
        }
    }

    pub fn from_bytes(mut b: Bytes) -> Result<Self, TryBufError> {
        Ok(if b.len() < 4 {
            Value::Unknown(b)
        } else if (b[0] != 0 || b[1] != 0) && (b[2] == 0 && b[3] == 0) {
            Value::Int(b.try_get_u16_le()?)
        } else {
            Value::Float(b.try_get_f32_le()?)
        })
    }
}

#[cfg(feature = "debug")]
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = self.clone().into_bytes();
        match self {
            Value::Unknown(u) => {
                let float = if b.len() >= 4 {
                    Some(b.clone().get_f32_le())
                } else {
                    None
                };

                let i = b[0];
                write!(
                    f,
                    "Value {{ Bytes: {:x?} (Int: {:?} | Float: {:?}) }}",
                    u.as_ref(),
                    i,
                    float
                )
            }
            &Value::Float(val) => {
                write!(f, "Value {{ Float: {:?} (Bytes: {:x?}) }}", val, b.as_ref())
            }
            &Value::Int(val) => {
                write!(f, "Value {{ Int: {:?} (Bytes: {:x?}) }}", val, b.as_ref())
            }
        }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Addr {
    pub val: u16,
    pub len: u8,
}

impl Addr {
    pub fn new(val: u16, len: u8) -> Self {
        Self { val, len }
    }

    pub fn read(buf: &mut Bytes, len: u8) -> Result<Self, TryBufError> {
        match len {
            1 => Self::read_u8(buf),
            2 => Self::read_u16(buf),
            _ => panic!("invalid address len"),
        }
    }

    pub fn read_u8(buf: &mut Bytes) -> Result<Self, TryBufError> {
        Ok(Self {
            val: buf.try_get_u8()? as u16,
            len: 1,
        })
    }

    pub fn read_u16(buf: &mut Bytes) -> Result<Self, TryBufError> {
        Ok(Self {
            val: buf.try_get_u16()?,
            len: 2,
        })
    }

    pub fn write(&self, buf: &mut BytesMut) {
        match self.len {
            1 => buf.put_u8(self.val as u8),
            2 => buf.put_u16(self.val),
            _ => panic!("invalid address len"),
        }
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub enum Commands {
    /// 0x31: Read hardware id
    ReadHardwareId,

    /// 0x14: Reads float data from a given base address. Max length is 14
    ReadFloats {
        addr: u16,
        len: u8,
    },

    /// 0x04: Writes byte data to the given address
    WriteMemory {
        addr: u16,
        data: BytesWrap,
    },

    /// 0x05: Reads byte data from the given address. Max read sizes are 61 bytes. (64 - crc - len - cmd)
    ReadMemory {
        addr: u16,
        size: u8,
    },

    /// 0x25: Sets the current configuration
    SetConfig {
        config: u8,
        reset: bool,
    },

    /// 0x34: Unary command to set the current source
    SetSource {
        source: u8,
    },

    /// 0x17 Unary command to set the master mute setting
    SetMute {
        value: bool,
    },

    /// 0x42: Set master volume
    SetVolume {
        value: Gain,
    },

    /// 0x30: Write biquad data
    WriteBiquad {
        addr: Addr,
        data: [f32; 5],
    },

    /// 0x19: Toggle biquad filter bypass
    WriteBiquadBypass {
        addr: Addr,
        value: bool,
    },

    /// 0x13: Write dsp data
    Write {
        addr: Addr,
        value: Value,
    },

    /// 0x39: Start FIR load
    FirLoadStart {
        index: u8,
    },

    /// 0x3a: FIR Data
    FirLoadData {
        index: u8,
        data: Vec<f32>, // Max 15 floats
    },

    /// 0x3b: FIR Data Completed
    FirLoadEnd,

    // 0x3f: DIRAC bypass
    DiracBypass {
        value: u8,
    },

    // Speculative commands
    /// 0x12: Seen when restoring a configuration
    BulkLoad {
        // Initial payload:
        // 04 88 97 13 0f 00 00
        // 04: 4 | (Addr&0x0F0000 >> 12)
        // 88: (Addr&0xFF00 >> 8)
        // 97: (Addr&0xFF)
        // 13: constant
        // 0f: constant
        // 00: constant
        // 00: constant
        payload: BytesWrap,
    },

    /// 0x06: Seen after 0x12 in configuration restore
    BulkLoadFilterData {
        // Initial packet:
        // 02 05 (addr+3 u16)
        payload: BytesWrap,
    },

    // 0x07: Seen in configuration restore
    Unk07 {
        payload: u8,
    },

    Unknown {
        cmd_id: u8,
        payload: BytesWrap,
    },
}

impl Commands {
    pub fn from_bytes(mut frame: Bytes) -> Result<Commands, ProtocolError> {
        Ok(match frame.try_get_u8()? {
            0x04 => Commands::WriteMemory {
                addr: frame.try_get_u16()?,
                data: BytesWrap(frame),
            },
            0x05 => Commands::ReadMemory {
                addr: frame.try_get_u16()?,
                size: frame.try_get_u8()?,
            },
            0x06 => Commands::BulkLoadFilterData {
                payload: BytesWrap(frame),
            },
            0x07 => Commands::Unk07 {
                payload: frame.try_get_u8()?,
            },
            0x12 => Commands::BulkLoad {
                payload: BytesWrap(frame),
            },
            0x13 => {
                frame.try_get_u8()?; // discard 0x80
                let len = if frame.len() >= 6 { 2 } else { 1 };
                Commands::Write {
                    addr: Addr::read(&mut frame, len)?,
                    value: Value::from_bytes(frame)?,
                }
            }
            0x14 => Commands::ReadFloats {
                addr: frame.try_get_u16()?,
                len: frame.try_get_u8()?,
            },
            0x17 => Commands::SetMute {
                value: frame.try_get_u8()? != 0,
            },
            0x19 => {
                let len = if frame.len() > 3 { 2 } else { 1 };
                Commands::WriteBiquadBypass {
                    value: frame.try_get_u8()? == 0x80,
                    addr: Addr::read(&mut frame, len)?,
                }
            }
            0x25 => Commands::SetConfig {
                config: frame.try_get_u8()?,
                reset: frame.try_get_u8()? != 0,
            },
            0x31 => Commands::ReadHardwareId {},
            0x30 => Commands::WriteBiquad {
                addr: {
                    frame.try_get_u8()?; // discard 0x80
                    Addr::read_u16(&mut frame)?
                },
                data: {
                    frame.try_get_u16()?; // discard 0x0000;
                    let mut data: [f32; 5] = Default::default();
                    for f in data.iter_mut() {
                        *f = frame.try_get_f32_le()?;
                    }
                    data
                },
            },
            0x34 => Commands::SetSource {
                source: frame.try_get_u8()?,
            },
            0x39 => Commands::FirLoadStart {
                index: frame.try_get_u8()?,
            },
            0x3a => Commands::FirLoadData {
                index: frame.try_get_u8()?,
                data: {
                    let mut data = Vec::with_capacity(15);
                    while frame.len() > 4 {
                        data.push(frame.try_get_f32_le()?);
                    }
                    data
                },
            },
            0x3b => Commands::FirLoadEnd,
            0x42 => Commands::SetVolume {
                value: frame.try_get_u8()?.into(),
            },
            cmd_id => Commands::Unknown {
                cmd_id,
                payload: BytesWrap(frame),
            },
        })
    }

    pub fn to_bytes(&self) -> Bytes {
        let mut f = BytesMut::with_capacity(64);

        match self {
            Commands::ReadHardwareId => {
                f.put_u8(0x31);
            }
            &Commands::ReadFloats { addr, len } => {
                f.put_u8(0x14);
                f.put_u16(addr);
                f.put_u8(len);
            }
            &Commands::ReadMemory { addr, size } => {
                f.put_u8(0x05);
                f.put_u16(addr);
                f.put_u8(size);
            }
            &Commands::WriteMemory { addr, ref data } => {
                f.put_u8(0x04);
                f.put_u16(addr);
                f.put(data.0.clone());
            }
            &Commands::SetConfig { config, reset } => {
                f.put_u8(0x25);
                f.put_u8(config);
                f.put_u8(reset as u8);
            }
            &Commands::SetSource { source } => {
                f.put_u8(0x34);
                f.put_u8(source);
            }
            &Commands::SetMute { value } => {
                f.put_u8(0x17);
                f.put_u8(value as u8);
            }
            &Commands::SetVolume { value } => {
                f.put_u8(0x42);
                f.put_u8((value).into());
            }
            &Commands::WriteBiquad { addr, data } => {
                f.put_u16(0x3080);
                addr.write(&mut f);
                f.put_u16(0x0000);
                for &coeff in data.iter() {
                    f.put_f32_le(coeff);
                }
            }
            &Commands::WriteBiquadBypass { addr, value } => {
                f.put_u8(0x19);
                f.put_u8(if value { 0x80 } else { 0x00 });
                addr.write(&mut f);
            }
            &Commands::Write { addr, ref value } => {
                f.put_u16(0x1380);
                addr.write(&mut f);
                f.put(value.clone().into_bytes());
            }

            &Commands::FirLoadStart { index } => {
                f.put_u8(0x39);
                f.put_u8(index);
            }
            &Commands::FirLoadData { index, ref data } => {
                f.put_u8(0x3a);
                f.put_u8(index);
                for &coeff in data {
                    f.put_f32_le(coeff);
                }
            }
            &Commands::FirLoadEnd => {
                f.put_u8(0x3b);
            }
            Commands::BulkLoad { payload } => {
                f.put_u8(0x12);
                f.put(payload.0.clone());
            }
            Commands::BulkLoadFilterData { payload } => {
                f.put_u8(0x06);
                f.put(payload.0.clone());
            }
            &Commands::DiracBypass { value } => {
                f.put_u8(0x3f);
                f.put_u8(value);
            }
            &Commands::Unk07 { payload } => {
                f.put_u8(0x07);
                f.put_u8(payload);
            }
            &Commands::Unknown {
                cmd_id,
                ref payload,
            } => {
                f.put_u8(cmd_id);
                f.put(payload.0.clone());
            }
        }
        f.freeze()
    }

    pub fn matches_response(&self, response: &Responses) -> bool {
        match self {
            &Commands::ReadMemory { addr, size } => {
                if let Responses::MemoryData(data) = response {
                    data.base == addr && data.data.len() == size as usize
                } else {
                    false
                }
            }
            &Commands::ReadFloats { addr, len } => {
                if let Responses::FloatData(data) = response {
                    data.base == addr && data.data.len() == len as usize
                } else {
                    false
                }
            }
            Commands::ReadHardwareId => matches!(response, Responses::HardwareId { .. }),
            Commands::SetConfig { .. } => matches!(response, Responses::ConfigChanged),
            Commands::FirLoadStart { .. } => matches!(response, Responses::FirLoadSize { .. }),
            &Commands::Unk07 { .. } => matches!(response, Responses::Unk02 { .. }),
            Commands::WriteMemory { .. }
            | Commands::SetSource { .. }
            | Commands::SetMute { .. }
            | Commands::SetVolume { .. }
            | Commands::WriteBiquad { .. }
            | Commands::WriteBiquadBypass { .. }
            | Commands::Write { .. }
            | Commands::FirLoadData { .. }
            | Commands::FirLoadEnd
            | Commands::BulkLoad { .. }
            | Commands::BulkLoadFilterData { .. }
            | Commands::DiracBypass { .. } => matches!(response, Responses::Ack),
            Commands::Unknown { .. } => true,
        }
    }

    pub fn mute(addr: u16, value: bool) -> Self {
        let value: u16 = if value {
            WriteInt::DISABLED
        } else {
            WriteInt::ENABLED
        };

        Commands::Write {
            addr: Addr::new(addr, 2),
            value: Value::Int(value),
        }
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub enum Responses {
    Ack,
    MemoryData(MemoryView),
    FloatData(FloatView),
    HardwareId {
        payload: BytesWrap,
    },
    FirLoadSize {
        size: u16,
    },

    /// Speculative commands
    ConfigChanged,

    // 0x02: Seen during configuration restore as a response to [`Commands::Unk07`]
    Unk02,

    Unknown {
        cmd_id: u8,
        payload: BytesWrap,
    },
}

impl Responses {
    pub fn from_bytes(mut frame: Bytes) -> Result<Responses, ProtocolError> {
        if frame.is_empty() {
            return Ok(Responses::Ack);
        }

        Ok(match frame[0] {
            0x05 => Responses::MemoryData(MemoryView::from_packet(frame)?),
            0x14 => Responses::FloatData(FloatView::from_packet(frame)?),
            0x31 => Responses::HardwareId {
                payload: {
                    frame.try_get_u8()?;
                    BytesWrap(frame)
                },
            },
            0x39 => Responses::FirLoadSize {
                size: {
                    frame.try_get_u8()?; // Consume command id
                    frame.try_get_u16()?
                },
            },
            0xab => Responses::ConfigChanged,
            0x02 => Responses::Unk02,
            cmd_id => Responses::Unknown {
                cmd_id,
                payload: BytesWrap(frame),
            },
        })
    }

    pub fn to_bytes(&self) -> Bytes {
        let mut f = BytesMut::with_capacity(64);
        match self {
            Responses::Ack => {}
            Responses::MemoryData(data) => {
                f.put_u8(0x05);
                f.put_u16(data.base);
                f.put(data.data.clone());
            }
            Responses::FloatData(data) => {
                f.put_u8(0x14);
                f.put_u16(data.base);

                for &item in &data.data {
                    f.put_f32_le(item);
                }
            }
            &Responses::Unknown {
                cmd_id,
                ref payload,
            } => {
                f.put_u8(cmd_id);
                f.put(payload.0.clone());
            }
            Responses::HardwareId { payload } => {
                f.put_u8(0x31);
                f.put(payload.0.clone());
            }
            &Responses::FirLoadSize { size } => {
                f.put_u8(0x39);
                f.put_u16(size);
            }
            Responses::ConfigChanged => {
                f.put_u8(0xab);
            }
            &Responses::Unk02 => {
                f.put_u8(0x02);
            }
        }
        f.freeze()
    }

    pub fn is_memory_view(&self) -> bool {
        matches!(self, Responses::MemoryData(_))
    }

    pub fn into_memory_view(self) -> Result<MemoryView, ProtocolError> {
        match self {
            Responses::MemoryData(m) => Ok(m),
            _ => Err(ProtocolError::UnexpectedResponseType),
        }
    }

    pub fn is_float_view(&self) -> bool {
        matches!(self, Responses::FloatData(_))
    }

    pub fn into_float_view(self) -> Result<FloatView, ProtocolError> {
        match self {
            Responses::FloatData(m) => Ok(m),
            _ => Err(ProtocolError::UnexpectedResponseType),
        }
    }

    pub fn is_hardware_id(&self) -> bool {
        matches!(self, Responses::HardwareId { .. })
    }

    pub fn into_hardware_id(self) -> Result<u8, ProtocolError> {
        match self {
            Responses::HardwareId { payload } => {
                Ok(*payload.last().ok_or(ProtocolError::MalformedHardwareId)?)
            }
            _ => Err(ProtocolError::UnexpectedResponseType),
        }
    }

    pub fn is_ack(&self) -> bool {
        matches!(self, Responses::Ack)
    }

    pub fn into_ack(self) -> Result<(), ProtocolError> {
        match self {
            Responses::Ack => Ok(()),
            _ => Err(ProtocolError::UnexpectedResponseType),
        }
    }

    pub fn is_config_changed(&self) -> bool {
        matches!(self, Responses::ConfigChanged)
    }

    pub fn into_config_changed(self) -> Result<(), ProtocolError> {
        match self {
            Responses::ConfigChanged => Ok(()),
            _ => Err(ProtocolError::UnexpectedResponseType),
        }
    }

    pub fn is_fir_size(&self) -> bool {
        matches!(self, Responses::FirLoadSize { .. })
    }

    pub fn into_fir_size(self) -> Result<u16, ProtocolError> {
        match self {
            Responses::FirLoadSize { size } => Ok(size),
            _ => Err(ProtocolError::UnexpectedResponseType),
        }
    }
}

/// Parsable response type
pub trait UnaryResponse: Sized {
    fn from_packet(packet: Bytes) -> Result<Self, ProtocolError>;
}

#[derive(Copy, Clone, PartialEq, Default)]
#[cfg_attr(feature = "debug", derive(Debug))]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize, schemars::JsonSchema)
)]
/// A gain between the minimum and maximum allowed values
pub struct Gain(pub f32);

impl Gain {
    pub const MIN: f32 = -127.;
    pub const MAX: f32 = 0.;
}

impl From<Gain> for u8 {
    fn from(val: Gain) -> Self {
        (val.0.abs() * 2.) as u8
    }
}

impl From<u8> for Gain {
    fn from(val: u8) -> Self {
        Self(-0.5 * (val as f32))
    }
}

impl FromStr for Gain {
    type Err = <f32 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Gain(<f32 as FromStr>::from_str(s)?))
    }
}
/// Memory views can be extended with multiple contiguous reads
pub trait ExtendView {
    fn extend_with(&mut self, other: Self) -> Result<(), ExtendError>;
}

#[cfg_attr(feature = "debug", derive(Debug, Error))]
pub enum ExtendError {
    #[cfg_attr(feature = "debug", error("the corresponding bases do not align"))]
    MismatchingBases,
}

/// A contiguous view of floats read from the device
#[derive(Clone, Default)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct FloatView {
    pub base: u16,
    pub data: Vec<f32>,
}

impl FloatView {
    pub fn get(&self, addr: u16) -> f32 {
        self.data[(addr - self.base) as usize]
    }
}

impl ExtendView for FloatView {
    fn extend_with(&mut self, other: Self) -> Result<(), ExtendError> {
        // Check that the `other` starts a the end of `self`
        let expected_start = self.base + (self.data.len() as u16);
        if other.base != expected_start {
            return Err(ExtendError::MismatchingBases);
        }

        self.data.extend(other.data.iter());

        Ok(())
    }
}

impl UnaryResponse for FloatView {
    fn from_packet(mut packet: Bytes) -> Result<Self, ProtocolError> {
        packet.try_get_u8()?; // Discard command id 0x14
        let base = packet.try_get_u16()?;
        let data = packet
            .chunks_exact(4)
            .map(|x| x.try_into().unwrap())
            .map(f32::from_le_bytes)
            .collect();

        Ok(FloatView { base, data })
    }
}

#[derive(Clone, Default)]
pub struct MemoryView {
    pub base: u16,
    pub data: Bytes,
}

impl MemoryView {
    pub fn read_at(&self, addr: u16, len: u8) -> Option<&'_ [u8]> {
        if addr < self.base || addr as usize > self.base as usize + self.data.len() {
            return None;
        }

        let start = (addr - self.base) as usize;
        let end = start + len as usize;

        if self.data.len() < end {
            return None;
        }

        Some(&self.data[start..end])
    }

    pub fn read_u8(&self, addr: u16) -> Option<u8> {
        Some(self.read_at(addr, 1)?[0])
    }

    pub fn read_u16(&self, addr: u16) -> Option<u16> {
        Some(u16::from_be_bytes(
            self.read_at(addr, 2)?.try_into().unwrap(),
        ))
    }
}

impl UnaryResponse for MemoryView {
    fn from_packet(mut packet: Bytes) -> Result<Self, ProtocolError> {
        packet.try_get_u8()?; // Discard command id 0x5
        let base = packet.try_get_u16()?;

        Ok(MemoryView { base, data: packet })
    }
}

#[cfg(feature = "debug")]
impl fmt::Debug for MemoryView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemoryView {{ base: {:04x?}, data: {:02x?} }}",
            self.base,
            self.data.as_ref()
        )
    }
}

impl ExtendView for MemoryView {
    fn extend_with(&mut self, other: Self) -> Result<(), ExtendError> {
        // Check that the `other` starts a the end of `self`
        let expected_start = self.base + (self.data.len() as u16);
        if other.base != expected_start {
            return Err(ExtendError::MismatchingBases);
        }

        let mut data: BytesMut = BytesMut::with_capacity(self.data.len() + other.data.len());
        data.extend(self.data.iter());
        data.extend(other.data.iter());

        // Truncate anything past 0xFFFF since it's probably garbage
        data.truncate((u16::MAX as usize) - (self.base as usize));

        self.data = data.freeze();

        Ok(())
    }
}

/// 0x13: Write an integer value
#[derive(Clone, Default)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct WriteInt;

impl WriteInt {
    pub const DISABLED: u16 = 1;
    pub const ENABLED: u16 = 2;
    pub const BYPASSED: u16 = 3;
}

/// Types that can be read from a contiguous memory representation
pub trait FromMemory<T: Sized>
where
    Self: Sized,
{
    fn from_memory(device_info: &DeviceInfo, view: &MemoryView) -> Result<Self>;
}

impl FromMemory<MasterStatus> for MasterStatus
where
    Self: Sized,
{
    fn from_memory(device_info: &DeviceInfo, view: &MemoryView) -> Result<Self> {
        Ok(Self {
            preset: view.read_u8(eeprom::PRESET),
            source: view
                .read_u8(eeprom::SOURCE)
                .or_else(|| view.read_u8(eeprom::SOURCE_ASYNC))
                .map(|id| super::Source::from_id(id, device_info)),
            volume: view.read_u8(eeprom::MASTER_VOLUME).map(Into::into),
            mute: view.read_u8(eeprom::MUTE).map(|x| x == 1),
            dirac: if device_info.supports_dirac() {
                view.read_u8(eeprom::DIRAC_BYPASS).map(|x| x == 0)
            } else {
                None
            },
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_read_reg() {
        let cmd = Commands::ReadMemory {
            addr: 0xffda,
            size: 4,
        };

        let mut req_packet = cmd.to_bytes();
        assert_eq!(req_packet.get_u8(), 0x05);
        assert_eq!(req_packet.get_u16(), 0xffda);
        assert_eq!(req_packet.get_u8(), 4);
        assert_eq!(req_packet.remaining(), 0);

        let response = Bytes::from_static(&[0x5, 0xff, 0xda, 0x1, 0x2, 0x3, 0x4, 0x0]);
        let memory = Responses::from_bytes(response)
            .ok()
            .unwrap()
            .into_memory_view()
            .ok()
            .unwrap();
        let data = memory.read_at(0xffda, 4);

        assert_eq!(data.unwrap(), &[0x1, 0x2, 0x3, 0x4]);
        assert_eq!(memory.read_u16(0xFFDA), Some(0x0102));
    }

    #[test]
    fn test_master_status() {
        let cmd = Commands::ReadMemory {
            addr: 0xffd8,
            size: 9,
        };

        let mut req_packet = cmd.to_bytes();
        assert_eq!(req_packet.get_u8(), 0x05);
        assert_eq!(req_packet.get_u16(), 0xffd8);
        assert_eq!(req_packet.get_u8(), 9);
        assert_eq!(req_packet.remaining(), 0);

        let response = Bytes::from_static(&[
            0x5, 0xff, 0xd8, 0x0, 0x1, 0x4f, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        ]);
        let memory = Responses::from_bytes(response)
            .ok()
            .unwrap()
            .into_memory_view()
            .ok()
            .unwrap();

        let device_info = DeviceInfo {
            hw_id: 10,
            dsp_version: 100,
            serial: 0,
        };
        let status = MasterStatus::from_memory(&device_info, &memory).unwrap();
        assert!(status.eq(&MasterStatus {
            preset: Some(0),
            source: Some(crate::Source::Toslink),
            volume: Some(Gain(-39.5)),
            mute: Some(false),
            dirac: None,
        }));
    }

    #[test]
    fn test_combine() {
        let mut f1 = FloatView {
            base: 0,
            data: (0u16..10).map(|x| x.into()).collect(),
        };

        let f2 = FloatView {
            base: 10,
            data: (10u16..20).map(|x| x.into()).collect(),
        };

        f1.extend_with(f2).ok().unwrap();
        assert_eq!(f1.base, 0);
        assert_eq!(f1.data.len(), 20);
        assert!(f1
            .data
            .into_iter()
            .eq((0u16..20).into_iter().map(|x| -> f32 { x.into() })));

        let mut m1 = MemoryView {
            base: 0,
            data: (0u8..10).collect(),
        };

        let m2 = MemoryView {
            base: 10,
            data: (10u8..20).collect(),
        };

        m1.extend_with(m2).ok().unwrap();
        assert_eq!(m1.base, 0);
        assert_eq!(m1.data.len(), 20);
        assert!(m1.data.into_iter().eq((0u8..20).into_iter()));
    }
}
