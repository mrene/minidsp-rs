use crate::{
    commands::{Addr, Value},
    FixedPoint,
};

/// Dialect represents the different encodings between devices
#[derive(Default)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Dialect {
    /// Length of addresses sent (either 3 (default) or 2)
    pub addr_encoding: AddrEncoding,

    /// Encoding for floating point values
    pub float_encoding: FloatEncoding,
}

impl Dialect {
    pub const fn const_default() -> Self {
        Self {
            addr_encoding: AddrEncoding::AddrLen3,
            float_encoding: FloatEncoding::Float32LE,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub enum AddrEncoding {
    AddrLen2 = 2,
    AddrLen3 = 3,
}

impl Default for AddrEncoding {
    fn default() -> Self {
        AddrEncoding::AddrLen3
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub enum FloatEncoding {
    Float32LE,
    FixedPoint,
}

impl Default for FloatEncoding {
    fn default() -> Self {
        FloatEncoding::Float32LE
    }
}

impl Dialect {
    pub fn addr(&self, value: u16) -> Addr {
        Addr::new(value, self.addr_encoding as u8)
    }

    pub fn float(&self, value: f32) -> Value {
        match self.float_encoding {
            FloatEncoding::Float32LE => Value::Float(value),
            FloatEncoding::FixedPoint => Value::FixedPoint(FixedPoint::from_f32(value)),
        }
    }

    pub fn db(&self, value: f32) -> Value {
        match self.float_encoding {
            FloatEncoding::Float32LE => Value::Float(value),
            FloatEncoding::FixedPoint => Value::FixedPoint(FixedPoint::from_db(value)),
        }
    }

    pub fn int(&self, value: u16) -> Value {
        Value::Int(value)
    }

    // FIXME: Don't rely on addr len here
    pub fn delay(&self, num_samples: u32) -> Value {
        match self.addr_encoding {
            AddrEncoding::AddrLen2 => Value::Int32(num_samples),
            AddrEncoding::AddrLen3 => Value::Int(num_samples as _),
        }
    }

    pub fn mute(&self, mute: bool) -> Value {
        match self.addr_encoding {
            AddrEncoding::AddrLen2 => Value::Int32(if mute { 0x0 } else { 0x0080_0000 }),
            AddrEncoding::AddrLen3 => Value::Int(if mute { 0x1 } else { 0x2 }),
        }
    }

    pub fn invert(&self, value: bool) -> Value {
        match self.addr_encoding {
            AddrEncoding::AddrLen2 => Value::Int32(if value { 0xFF80_0000 } else { 0x0080_0000 }),
            AddrEncoding::AddrLen3 => Value::Int(value as _),
        }
    }
}
