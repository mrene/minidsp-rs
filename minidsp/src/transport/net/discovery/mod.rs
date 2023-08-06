//! Enables discovery and advertisement of tcp servers.
//! The packet format is compatible with the official apps.
use std::{convert::TryInto, net::Ipv4Addr};

use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

pub mod client;
pub mod server;

pub const DISCOVERY_PORT: u16 = 3999;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryPacket {
    pub mac_address: [u8; 6],
    pub ip_address: Ipv4Addr,
    pub hwid: u8,
    pub dsp_id: u8,
    pub sn: u16,
    pub fw_major: u8,
    pub fw_minor: u8,
    pub hostname: String,
}

impl DiscoveryPacket {
    pub fn parse(packet: &[u8]) -> Result<Self> {
        if packet.len() < 36 {
            return Err(anyhow!("packet too short"));
        }

        let hostname_len = packet[35] as usize;
        if packet.len() < 36 + hostname_len {
            return Err(anyhow!("name doesn't fit inside packet"));
        }

        let ip_array: [u8; 4] = packet[14..18].try_into().unwrap();
        let p = DiscoveryPacket {
            hwid: packet[18],
            dsp_id: packet[21],
            sn: ((packet[22] as u16) << 8) | (packet[23] as u16),
            fw_major: packet[19],
            fw_minor: packet[20],
            ip_address: Ipv4Addr::from(ip_array),
            mac_address: packet[6..12].try_into().unwrap(),
            hostname: String::from_utf8_lossy(&packet[36..36 + packet[35] as usize]).to_string(),
        };

        Ok(p)
    }

    pub fn to_bytes(&self) -> Bytes {
        let mut p = BytesMut::with_capacity(64);
        p.put_slice(&[0x80, 0x0, 0x05, 0xA0]);
        p.resize(35, 0);
        self.mac_address.as_ref().copy_to_slice(&mut p[6..12]);
        self.ip_address
            .octets()
            .as_ref()
            .copy_to_slice(&mut p[14..18]);
        p[18] = self.hwid;
        p[19] = self.fw_major;
        p[20] = self.fw_minor;
        p[21] = self.dsp_id;
        p[22] = (self.sn >> 8) as u8;
        p[23] = (self.sn & 0xFF) as u8;
        if self.hostname.len() > u8::MAX as usize {
            panic!("hostname was above max length")
        }
        p.put_u8(self.hostname.len() as u8);
        p.put_slice(self.hostname.as_bytes());
        p.freeze()
    }
}

pub struct DiscoveryPacketCodec {}

impl Decoder for DiscoveryPacketCodec {
    type Item = DiscoveryPacket;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let packet = DiscoveryPacket::parse(src.as_ref());
        let ret = match packet {
            Ok(p) => Ok(Some(p)),

            // Discard bogus or empty frames
            Err(_) => Ok(None),
        };
        src.clear();
        ret
    }
}

impl Encoder<DiscoveryPacket> for DiscoveryPacketCodec {
    type Error = anyhow::Error;

    fn encode(&mut self, item: DiscoveryPacket, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend(item.to_bytes().iter());
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::net::Ipv4Addr;

    use anyhow::Result;
    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_bytes_parse() -> Result<()> {
        let p = DiscoveryPacket {
            mac_address: [10, 20, 30, 40, 50, 60],
            ip_address: Ipv4Addr::new(192, 168, 1, 100),
            hwid: 222,
            dsp_id: 51,
            sn: 1234,
            fw_major: 1,
            fw_minor: 2,
            hostname: "Living room TV".to_string(),
        };

        let b = p.to_bytes();
        let parsed = DiscoveryPacket::parse(&b)?;
        assert_eq!(p, parsed);
        Ok(())
    }

    #[test]
    fn test_parse() -> Result<()> {
        let a = Bytes::from_static(&[
            0x80, 0x12, 0x05, 0xa0, 0x00, 0x00, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x00,
            0xc0, 0xa8, 0x79, 0x8d, 0xde, 0x03, 0x00, 0x33, 0xab, 0x64, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xac, 0x00, 0x00, 0x00, 0x00, 0x08, 0x50, 0x52, 0x4f, 0x44, 0x55, 0x43,
            0x54, 0x00, 0xfc, 0xa1, 0x51, 0x98, 0x43, 0x69, 0xaa, 0x6c, 0x76, 0xac, 0xba, 0xaf,
            0x37, 0x83, 0xbe, 0x61, 0xf5, 0x69, 0xd0, 0x98, 0x1c, 0xe0, 0x95, 0xf2, 0x6b, 0x81,
            0xd8, 0x60,
        ]);
        let b = Bytes::from_static(&[
            0x80, 0x12, 0x05, 0xa0, 0x00, 0x00, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x00,
            0xc0, 0xa8, 0x79, 0x8e, 0xde, 0x03, 0x00, 0x33, 0xab, 0x65, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xac, 0x00, 0x00, 0x00, 0x00, 0x08, 0x50, 0x52, 0x4f, 0x44, 0x55, 0x43,
            0x54, 0x00, 0xfc, 0xa1, 0x51, 0x98, 0x43, 0x69, 0xaa, 0x6c, 0x76, 0xac, 0xba, 0xaf,
            0x37, 0x83, 0xbe, 0x61, 0xf5, 0x69, 0xd0, 0x98, 0x1c, 0xe0, 0x95, 0xf2, 0x6b, 0x81,
            0xd8, 0x60,
        ]);
        let c = Bytes::from_static(&[
            0x80, 0x12, 0x05, 0xa0, 0x00, 0x00, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x00,
            0xc0, 0xa8, 0x79, 0x8f, 0xde, 0x03, 0x00, 0x33, 0xab, 0x66, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xac, 0x00, 0x00, 0x00, 0x00, 0x08, 0x50, 0x52, 0x4f, 0x44, 0x55, 0x43,
            0x54, 0x00, 0xfc, 0xa1, 0x51, 0x98, 0x43, 0x69, 0xaa, 0x6c, 0x76, 0xac, 0xba, 0xaf,
            0x37, 0x83, 0xbe, 0x61, 0xf5, 0x69, 0xd0, 0x98, 0x1c, 0xe0, 0x95, 0xf2, 0x6b, 0x81,
            0xd8, 0x60,
        ]);

        println!("a {:?}", DiscoveryPacket::parse(&a)?);
        println!("b {:?}", DiscoveryPacket::parse(&b)?);
        println!("c {:?}", DiscoveryPacket::parse(&c)?);
        Ok(())
    }
}
