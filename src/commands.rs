use std::convert::TryInto;

use crate::transport::Transport;
use crate::Source;

pub trait FromMemory<T: Sized>
where
    Self: Sized,
{
    fn from_memory(view: &MemoryView) -> Result<Self, failure::Error>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct MasterStatus {
    /// Active configuration preset
    pub preset: u8,

    /// Active source
    pub source: Source,

    /// Volume in dB [-127, 0]
    pub volume: Gain,

    /// Mute status
    pub mute: bool,
}

impl FromMemory<MasterStatus> for MasterStatus
where
    Self: Sized,
{
    fn from_memory(view: &MemoryView) -> Result<Self, failure::Error> {
        Ok(Self {
            preset: view.read_u8(0xffd8),
            source: view.read_u8(0xffd9).try_into()?,
            volume: view.read_u8(0xffda).into(),
            mute: view.read_u8(0xffdb) == 1,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Gain(pub f32);

impl Gain {
    pub const MIN: f32 = -127f32;
    pub const MAX: f32 = 0f32;
}

impl Into<u8> for Gain {
    fn into(self) -> u8 {
        (self.0.abs() * 2.) as u8
    }
}

impl From<u8> for Gain {
    fn from(val: u8) -> Self {
        Self(-1. * (val as f32) / 2.)
    }
}

pub struct SetVolume {
    pub value: Gain,
}

impl SetVolume {
    pub fn new(value: Gain) -> Self {
        Self { value }
    }
    pub fn execute(&self, transport: &mut dyn Transport) -> Result<(), failure::Error> {
        let _ = transport.roundtrip(&[0x42, self.value.into()])?;
        Ok(())
    }
}

pub struct SetMute {
    pub value: bool,
}

impl SetMute {
    pub fn new(value: bool) -> Self {
        SetMute { value }
    }

    pub fn execute(&self, transport: &mut dyn Transport) -> Result<(), failure::Error> {
        let value = self.value as u8;
        let _ = transport.roundtrip(&[0x17, value])?;
        Ok(())
    }
}

pub struct SetSource {
    source: Source,
}

impl SetSource {
    pub fn new(source: Source) -> Self {
        Self { source }
    }

    pub fn execute(&self, transport: &mut dyn Transport) -> Result<(), failure::Error> {
        let _ = transport.roundtrip(&[0x34, self.source.into()])?;
        Ok(())
    }
}

/// Reads data from the given address. Max read sizes are 61 bytes. (64 - crc - len - cmd)
pub struct ReadMemory {
    pub addr: u16,
    pub size: u8,
}

impl ReadMemory {
    pub fn to_bytes(&self) -> [u8; 4] {
        let mut cmd: [u8; 4] = [0x05, 0x0, 0x0, self.size];
        cmd[1..3].copy_from_slice(self.addr.to_be_bytes().as_ref());
        cmd
    }

    pub fn execute(&self, transport: &mut dyn Transport) -> Result<MemoryView, failure::Error> {
        let cmd = self.to_bytes();
        let response = transport.roundtrip(&cmd)?;
        if !&cmd[1..3].eq(&response[1..3]) {
            Err(failure::format_err!("Malformed response"))
        } else {
            Ok(MemoryView {
                base: self.addr,
                size: self.size,
                data: Vec::from(&response[3..]),
            })
        }
    }
}

pub struct MemoryView {
    pub base: u16,
    pub size: u8,
    data: Vec<u8>,
}

impl MemoryView {
    pub fn read_at(&self, addr: u16, len: u8) -> &'_ [u8] {
        let start = (addr - self.base) as usize;
        let end = start + len as usize;

        &self.data[start..end]
    }

    pub fn read_u8(&self, addr: u16) -> u8 {
        self.read_at(addr, 1)[0]
    }

    pub fn read_u16(&self, addr: u16) -> u16 {
        u16::from_be_bytes(self.read_at(addr, 2).try_into().unwrap())
    }

    pub fn read_f32(&self, addr: u16) -> f32 {
        f32::from_be_bytes(self.read_at(addr, 4).try_into().unwrap())
    }
}

impl std::ops::Index<u16> for MemoryView {
    type Output = u8;

    fn index(&self, index: u16) -> &Self::Output {
        let index = index - self.base;
        &self.data[index as usize]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct Mock(Vec<u8>);

    impl Transport for Mock {
        fn roundtrip(&mut self, _: &[u8]) -> Result<Vec<u8>, failure::Error> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn test_read_reg() {
        let cmd = ReadMemory {
            addr: 0xffda,
            size: 4,
        };
        assert_eq!(cmd.to_bytes(), vec![0x05, 0xff, 0xda, 0x04].as_ref());

        let mut mock = Mock(vec![0x5, 0xff, 0xda, 0x1, 0x2, 0x3, 0x4]);
        let resp = cmd.execute(&mut mock).unwrap();
        let data = resp.read_at(0xffda, 4);

        assert_eq!(data, &[0x1, 0x2, 0x3, 0x4]);
        assert_eq!(resp.read_u16(0xFFDA), 0x0102);
        assert!(
            (resp.read_f32(0xFFDA) - f32::from_be_bytes([0x01, 0x02, 0x03, 0x04])).abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn test_master_status() {
        let mut mock = Mock(vec![0x5, 0xff, 0xd8, 0x0, 0x1, 0x4f, 0x0]);
        let cmd = ReadMemory {
            addr: 0xffd8,
            size: 4,
        };

        let memory = cmd.execute(&mut mock).unwrap();

        let status = MasterStatus::from_memory(&memory).unwrap();
        assert_eq!(
            status,
            MasterStatus {
                preset: 0,
                source: Source::Toslink,
                volume: Gain(-39.5),
                mute: false,
            }
        );
    }
}
