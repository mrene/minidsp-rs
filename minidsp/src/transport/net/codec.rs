//! A codec for "quirky" length delimited encoding, as done by minidsp products.
//! Most frames are encoded with a length prefix, but since the applications do not separate frames by length, they
//! instead rely on the OS's packetization to feed a single frame in every read request. This works most of the time,
//! but on certain platforms (such as iOS), they zero-pad the packets to a 64 byte length in order to match the HID report
//! size. Their proxy component imposes a read buffer size of 64 bytes which sort of solves the problem.
//!
//! Instead of relying on this behaviour, this codec attempts to decode by length prefix, but discards zero-padded data
//! if it is found after a frame boundary.
//!
//! There is a major difference on the client and server side.
//! On the server side, the length prefix doesn't count as part of the frame size (either that, or the trailing crc doesn't count, either way, there's an extra byte).
//! On the client side, the length prefix includes itself, and there is no CRC byte. [01] is a valid zero-length response used for acknowledgments.
//!

use std::io;

use anyhow::Result;
use bytes::{Buf, Bytes};
use tokio_util::codec::{Decoder, Encoder};

#[derive(Copy, Clone)]
pub struct Codec {
    server: bool,
}

impl Codec {
    pub fn new_server() -> Self {
        Codec { server: true }
    }

    pub fn new_client() -> Self {
        Codec { server: false }
    }
}

impl Decoder for Codec {
    type Item = Bytes;
    type Error = io::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        while !src.is_empty() {
            let additional_length = if self.server { 1 } else { 0 };

            if src[0] != 0 {
                let n = src[0] as usize + additional_length;
                return if src.len() >= n {
                    Ok(Some(src.split_to(n).freeze()))
                } else {
                    Ok(None)
                };
            } else {
                // Skip zero-padding
                let zeroes = src.iter().take_while(|x| **x == 0).count();
                src.advance(zeroes);
            }
        }

        Ok(None)
    }
}

impl Encoder<Bytes> for Codec {
    type Error = io::Error;

    fn encode(&mut self, item: Bytes, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        dst.extend(item);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bytes::{Bytes, BytesMut};
    use tokio_util::codec::Decoder;

    use super::*;

    #[test]
    fn test() {
        let packet =
            Bytes::from_static(&[0x09, 0x13, 0x80, 0x00, 0x1f, 0x9a, 0x99, 0x99, 0x3e, 0xc5]);
        let mut padded_packets = {
            let mut bm = BytesMut::new();
            bm.extend(packet.iter());
            bm.resize(64, 0);
            bm.extend(packet.iter());
            bm
        };

        let mut codec = Codec::new_server();

        let decoded = codec.decode(&mut padded_packets).unwrap().unwrap();
        assert!(decoded.iter().cloned().eq(packet.iter().cloned()));

        let decoded = codec.decode(&mut padded_packets).unwrap().unwrap();
        assert!(decoded.iter().cloned().eq(packet.iter().cloned()));

        let decoded = codec.decode(&mut padded_packets).unwrap();
        assert!(decoded.is_none());

        {
            let mut bm = BytesMut::new();
            bm.extend_from_slice(&packet[0..4]);

            let decoded = codec.decode(&mut bm).unwrap();
            assert!(decoded.is_none());

            bm.extend_from_slice(&packet[4..]);

            let decoded = codec.decode(&mut bm).unwrap();
            assert!(decoded.is_some());
        }
    }

    #[test]
    fn test_client() {
        let mut codec = Codec::new_client();
        let mut packet = BytesMut::from(&[0x01][..]);
        let decoded = codec.decode(&mut packet).unwrap().unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0], 1);

        let mut packet = BytesMut::from(&[0x14, 0x14, 0x00, 0x46][..]);
        packet.resize(20, 0);

        let decoded = codec.decode(&mut packet).unwrap().unwrap();
        assert_eq!(decoded.len(), 20);
    }
}
