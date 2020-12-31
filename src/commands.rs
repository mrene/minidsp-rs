use crate::transport::{MiniDSPError, Transport};
use crate::{packet, Source};
use anyhow::Result;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::convert::TryInto;
use std::str::FromStr;
use thiserror::Error;

/// Trait representing a command that has a bytes representation and a parsed response.
pub trait UnaryCommand {
    type Response: UnaryResponse;

    fn request_packet(&self) -> Bytes;
    fn response_matches(&self, _packet: &[u8]) -> bool {
        true
    }
    fn parse_response(&self, packet: Bytes) -> Self::Response {
        Self::Response::from_packet(packet)
    }
}

/// Parsable response type (TODO: Would `Decoder` fit here instead?)
pub trait UnaryResponse {
    fn from_packet(packet: Bytes) -> Self;
}

impl UnaryResponse for () {
    fn from_packet(_packet: Bytes) -> Self {}
}

impl UnaryResponse for Bytes {
    fn from_packet(packet: Bytes) -> Self {
        packet
    }
}

/// Acquire an exclusive lock to the transport,
/// send a command and wait for its response.
/// (to cancel: drop the returned future)
pub async fn roundtrip<C>(
    transport: &dyn Transport,
    command: C,
) -> Result<C::Response, MiniDSPError>
where
    C: UnaryCommand,
{
    let mut receiver = transport.subscribe();
    let mut sender = transport.send_lock().await;

    sender.send(packet::frame(command.request_packet())).await?;

    while let Ok(frame) = receiver.recv().await {
        if let Ok(p) = packet::unframe(frame) {
            if command.response_matches(&p) {
                return Ok(command.parse_response(p));
            }
        }
    }

    // TODO: Handle other error cases
    Err(MiniDSPError::MalformedResponse)
}

/// Types that can be read from a contiguous memory representation
pub trait FromMemory<T: Sized>
where
    Self: Sized,
{
    fn from_memory(view: &MemoryView) -> Result<Self>;
}

#[derive(Debug, Clone, PartialEq)]
/// The current settings applying to all outputs
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
    fn from_memory(view: &MemoryView) -> Result<Self> {
        Ok(Self {
            preset: view.read_u8(0xffd8),
            source: view.read_u8(0xffd9).try_into()?,
            volume: view.read_u8(0xffda).into(),
            mute: view.read_u8(0xffdb) == 1,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
/// A gain between the minimum and maximum allowed values
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

impl FromStr for Gain {
    type Err = <f32 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Gain(<f32 as FromStr>::from_str(s)?))
    }
}

/// [0x42] Unary command to set the master volume
pub struct SetVolume {
    pub value: Gain,
}

impl SetVolume {
    pub fn new(value: Gain) -> Self {
        Self { value }
    }
}

impl UnaryCommand for SetVolume {
    type Response = ();
    fn request_packet(&self) -> Bytes {
        Bytes::from(vec![0x42, self.value.into()])
    }
}

/// [0x17] Unary command to set the master mute setting
pub struct SetMute {
    pub value: bool,
}

impl SetMute {
    pub fn new(value: bool) -> Self {
        SetMute { value }
    }
}

impl UnaryCommand for SetMute {
    type Response = ();

    fn request_packet(&self) -> Bytes {
        Bytes::from(vec![0x17, self.value as u8])
    }
}

/// [0x25] Sets the current configuration
pub struct SetConfig {
    config: u8,
    reset: bool,
}

impl SetConfig {
    pub fn new(config: u8) -> Self {
        SetConfig {
            config,
            reset: true,
        }
    }
}

impl UnaryCommand for SetConfig {
    type Response = ();

    fn request_packet(&self) -> Bytes {
        Bytes::from(vec![0x25, self.config, self.reset as u8])
    }
}

/// [0x34] Unary command to set the current source
pub struct SetSource {
    source: Source,
}

impl SetSource {
    pub fn new(source: Source) -> Self {
        Self { source }
    }
}

impl UnaryCommand for SetSource {
    type Response = ();

    fn request_packet(&self) -> Bytes {
        Bytes::from(vec![0x34, self.source.into()])
    }
}

/// Custom unary command used for sending custom commmands for debugging purposes
pub struct CustomUnaryCommand {
    request: Bytes,
}

impl CustomUnaryCommand {
    pub fn new(request: Bytes) -> Self {
        CustomUnaryCommand { request }
    }
}

impl UnaryCommand for CustomUnaryCommand {
    type Response = Bytes;

    fn request_packet(&self) -> Bytes {
        self.request.clone()
    }
}

/// [0x05] Reads byte data from the given address. Max read sizes are 61 bytes. (64 - crc - len - cmd)
pub struct ReadMemory {
    pub addr: u16,
    pub size: u8,
}

impl ReadMemory {
    pub fn new(addr: u16, size: u8) -> Self {
        ReadMemory { addr, size }
    }
}

impl UnaryCommand for ReadMemory {
    type Response = MemoryView;

    fn request_packet(&self) -> Bytes {
        let mut cmd = BytesMut::with_capacity(4);
        cmd.put_u8(0x05);
        cmd.put_u16(self.addr);
        cmd.put_u8(self.size);
        cmd.freeze()
    }

    fn response_matches(&self, packet: &[u8]) -> bool {
        if !packet.starts_with(&[0x05]) {
            return false;
        }

        let mut b = Bytes::copy_from_slice(packet);
        b.get_u8();
        self.addr == b.get_u16() && self.size == (b.remaining() as u8)
    }
}

/// [0x14] Reads float data from a given base address. Max length is 14
pub struct ReadFloats {
    pub addr: u16,
    pub len: u8,
}

impl ReadFloats {
    pub fn new(addr: u16, len: u8) -> Self {
        if len > 14 {
            panic!("length too big")
        }
        ReadFloats { addr, len }
    }
}

impl UnaryCommand for ReadFloats {
    type Response = FloatView;

    fn request_packet(&self) -> Bytes {
        let mut cmd = BytesMut::with_capacity(4);
        cmd.put_u8(0x14);
        cmd.put_u16(self.addr);
        cmd.put_u8(self.len);
        cmd.freeze()
    }

    fn response_matches(&self, packet: &[u8]) -> bool {
        if !packet.starts_with(&[0x14]) {
            return false;
        }

        let mut b = Bytes::copy_from_slice(packet);
        b.get_u8();
        self.addr == b.get_u16() && self.len == ((b.remaining() as u8) / 4)
    }
}

/// Memory views can be extended with multiple contiguous reads
pub trait ExtendView {
    fn extend_with(&mut self, other: Self) -> Result<(), ExtendError>;
}

#[derive(Error, Debug)]
pub enum ExtendError {
    #[error("the corresponding bases do not align")]
    MismatchingBases,
}

/// A contiguous view of floats read from the device
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
    fn from_packet(mut packet: Bytes) -> Self {
        packet.get_u8(); // Discard command id 0x14
        let base = packet.get_u16();
        let data = packet
            .chunks_exact(4)
            .map(|x| x.try_into().unwrap())
            .map(f32::from_le_bytes)
            .collect();

        FloatView { base, data }
    }
}

/// A contiguous bytes view read from the device
pub struct MemoryView {
    pub base: u16,
    pub data: Bytes,
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
}

impl UnaryResponse for MemoryView {
    fn from_packet(mut packet: Bytes) -> Self {
        packet.get_u8(); // Discard command id 0x5
        let base = packet.get_u16();
        MemoryView { base, data: packet }
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
        self.data = data.freeze();

        Ok(())
    }
}

/// [0x13] Write float data
pub struct WriteFloat {
    pub addr: u16,
    pub value: f32,
}

impl WriteFloat {
    pub fn new(addr: u16, value: f32) -> Self {
        WriteFloat { addr, value }
    }
}

impl UnaryCommand for WriteFloat {
    type Response = ();

    fn request_packet(&self) -> Bytes {
        let mut b = BytesMut::with_capacity(8);
        b.put_u8(0x13);
        b.put_u8(0x80);
        b.put_u16(self.addr);
        b.put_f32_le(self.value);

        b.freeze()
    }
}

/// [0x13] Write an integer value
pub struct WriteInt {
    pub addr: u16,
    pub value: u8,
}

impl WriteInt {
    pub const DISABLED: u8 = 1;
    pub const ENABLED: u8 = 2;
    pub const BYPASSED: u8 = 3;

    pub fn new(addr: u16, value: u8) -> Self {
        WriteInt { addr, value }
    }

    pub fn mute(addr: u16, value: bool) -> Self {
        let value = if value {
            WriteInt::DISABLED
        } else {
            WriteInt::ENABLED
        };

        WriteInt { addr, value }
    }
}

impl UnaryCommand for WriteInt {
    type Response = ();

    fn request_packet(&self) -> Bytes {
        let mut b = BytesMut::with_capacity(16);
        b.put_slice(&[0x13, 0x80]);
        b.put_u16(self.addr);
        b.put_u8(self.value);
        b.put_slice(&[0x00, 0x00, 0x00]);
        b.freeze()
    }
}

/// [0x30] Write biquad data
pub struct WriteBiquad {
    pub addr: u16,
    pub data: [f32; 5],
}

impl WriteBiquad {
    pub fn new(addr: u16, data: [f32; 5]) -> Self {
        WriteBiquad { addr, data }
    }
}

impl UnaryCommand for WriteBiquad {
    type Response = ();

    fn request_packet(&self) -> Bytes {
        let mut b = BytesMut::with_capacity(64);
        b.put_slice(&[0x30, 0x80]);
        b.put_u16(self.addr);
        b.put_u16(0x0000);
        for f in self.data.iter() {
            b.put_f32_le(*f);
        }
        b.freeze()
    }
}

/// [0x19] Toggle biquad filter bypass
pub struct WriteBiquadBypass {
    pub addr: u16,
    pub value: bool,
}

impl WriteBiquadBypass {
    pub fn new(addr: u16, value: bool) -> Self {
        WriteBiquadBypass { addr, value }
    }
}

impl UnaryCommand for WriteBiquadBypass {
    type Response = ();

    fn request_packet(&self) -> Bytes {
        let mut p = BytesMut::with_capacity(16);
        p.put_u8(0x19);
        p.put_u8(if self.value { 0x80 } else { 0x00 });
        p.put_u16(self.addr);
        p.freeze()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_read_reg() {
        let cmd = ReadMemory {
            addr: 0xffda,
            size: 4,
        };

        let mut req_packet = cmd.request_packet();
        assert_eq!(req_packet.get_u8(), 0x05);
        assert_eq!(req_packet.get_u16(), 0xffda);
        assert_eq!(req_packet.get_u8(), 4);
        assert_eq!(req_packet.remaining(), 0);

        let response = Bytes::from_static(&[0x5, 0xff, 0xda, 0x1, 0x2, 0x3, 0x4]);
        let memory = cmd.parse_response(response);
        let data = memory.read_at(0xffda, 4);

        assert_eq!(data, &[0x1, 0x2, 0x3, 0x4]);
        assert_eq!(memory.read_u16(0xFFDA), 0x0102);
    }

    #[test]
    fn test_master_status() {
        let cmd = ReadMemory {
            addr: 0xffd8,
            size: 4,
        };

        let mut req_packet = cmd.request_packet();
        assert_eq!(req_packet.get_u8(), 0x05);
        assert_eq!(req_packet.get_u16(), 0xffd8);
        assert_eq!(req_packet.get_u8(), 4);
        assert_eq!(req_packet.remaining(), 0);

        let response = Bytes::from_static(&[0x5, 0xff, 0xd8, 0x0, 0x1, 0x4f, 0x0]);
        let memory = cmd.parse_response(response);
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

        f1.extend_with(f2).unwrap();
        assert_eq!(f1.base, 0);
        assert_eq!(f1.data.len(), 20);
        assert!(f1
            .data
            .into_iter()
            .eq((0u16..20).into_iter().map(|x| x.into())));

        let mut m1 = MemoryView {
            base: 0,
            data: (0u8..10).collect(),
        };

        let m2 = MemoryView {
            base: 10,
            data: (10u8..20).collect(),
        };

        m1.extend_with(m2).unwrap();
        assert_eq!(m1.base, 0);
        assert_eq!(m1.data.len(), 20);
        assert!(m1.data.into_iter().eq((0u8..20).into_iter()));
    }

    #[test]
    fn biquad_test() {
        let b = WriteBiquad {
            addr: 0,
            data: [1.0, 0.1, 0.2, 0.3, 0.4],
        };

        for (i, f) in b.data.iter().enumerate() {
            println!("{}: {} {:02x?}", i, f, f.to_le_bytes().as_ref())
        }

        println!("{:?}", f32::from_le_bytes([0x01, 0x00, 0x00, 0x00]));
        println!("{:?}", f32::from_le_bytes([0x02, 0x00, 0x00, 0x00]));
    }
}
