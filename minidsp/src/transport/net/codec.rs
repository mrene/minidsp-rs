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
//! On the client side, the length prefix includes itself, and there is no CRC byte. \[01\] is a valid zero-length response used for acknowledgments.

use std::io;

use anyhow::Result;
use bytes::{Buf, Bytes};
use tokio_util::codec::{Decoder, Encoder};

#[derive(Copy, Clone)]
pub struct Codec {
    server: bool,
    // If true, we have received a frame smaller than the fixed packet size, and we
    // should not discard extraneous data after a packet, even though the
    // total available data size is a multiple of the fixed packet size
    received_small_packet: bool,

    // Represents the size of a single packet, when in fixed length mode
    fixed_packet_size: Option<usize>,
}

impl Codec {
    pub fn new_server() -> Self {
        Codec {
            server: true,
            received_small_packet: false,
            fixed_packet_size: None,
        }
    }

    pub fn new_client() -> Self {
        Codec {
            server: false,
            received_small_packet: false,
            fixed_packet_size: None,
        }
    }
}

impl Decoder for Codec {
    type Item = Bytes;
    type Error = io::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // If we didn't get a small packet, we're operating in fixed packet size mode, and we do not use the packet's length header to determine packet boundaries.
        // See [`widg_nonzero_padding`] for more details.
        if !self.server && !self.received_small_packet && !src.is_empty() {
            if src.len() >= 64 {
                // The first packet received determines the length used by this specific device, unless it's smaller than an HID frame (64 bytes)
                if self.fixed_packet_size.is_none() {
                    // Because we can receive multiple packets at once, assume that the real size is < 100
                    // Valid packet sizes at this time are: 64 bytes (wi-dg), 70 bytes (PWR-ICE)
                    let mut len = src.len();
                    while len > 100 && len % 2 == 0 {
                        len /= 2
                    }
                    self.fixed_packet_size.replace(len);
                }

                let mut buf = src.split_to(self.fixed_packet_size.unwrap());
                return Ok(Some(buf.split_to(buf[0] as usize).freeze()));
            } else {
                // If a single received frame is not >= 64 bytes, drop out of this hack, as it may hinder
                // properly behaving servers under certain circumstances, where their packets could be 64 bytes too.
                self.received_small_packet = true
            }
        }

        while !src.is_empty() {
            let additional_length = usize::from(self.server);

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

    #[test]
    fn widg_nonzero_padding() {
        // A user reported some commands did not yield a valid response, and were stalling the device probing process.
        // From a provided pcap, it's clear that fixed-size 64 bytes frames are returned, with what appears to be
        // random memory filling the rest of the buffer. These really look like full HID frames, because they have the
        // incrementing last hex byte defining these.

        let parts = [
            "0531010c0ada01bb23f90100bb253dbb9419bb13b6bb2394f682f628986b040024bb440db4f6061c6c040032bb43ed3cf606f632bb12aabb1407bb5409f62810",
            "0505ffa164da01bb23f90100bb253dbb9419bb13b6bb2394f682f628986b040024bb440db4f6061c6c040032bb43ed3cf606f632bb12aabb1407bb5409f62811",
        ];

        let mut packet: BytesMut = parts
            .iter()
            .flat_map(|s| hex::decode(s).unwrap().into_iter())
            .collect();

        let mut codec = Codec::new_client();

        let decoded = codec.decode(&mut packet).unwrap().unwrap();
        assert_eq!(hex::encode(decoded), "0531010c0a");

        let decoded = codec.decode(&mut packet).unwrap().unwrap();
        assert_eq!(hex::encode(decoded), "0505ffa164");

        let decoded = codec.decode(&mut packet).unwrap();
        assert_eq!(decoded, None);
    }

    #[test]
    fn pwr_ice() {
        let parts = [
            "04310300ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffa3f40797f63302",
            "0505ffa133ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffa3f40797f6ab02",
        ];

        let mut packet: BytesMut = parts
            .iter()
            .flat_map(|s| hex::decode(s).unwrap().into_iter())
            .collect();

        let mut codec = Codec::new_client();

        let decoded = codec.decode(&mut packet).unwrap().unwrap();
        assert_eq!(hex::encode(decoded), "04310300");

        let decoded = codec.decode(&mut packet).unwrap().unwrap();
        assert_eq!(hex::encode(decoded), "0505ffa133");

        let decoded = codec.decode(&mut packet).unwrap();
        assert_eq!(decoded, None);
    }
}
