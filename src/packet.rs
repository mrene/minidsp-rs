use crate::transport::{MiniDSPError, MiniDSPError::MalformedResponse};
use bytes::{BufMut, Bytes, BytesMut};

// Formats an hid command
pub fn frame<T: AsRef<[u8]>>(packet: T) -> Bytes {
    let mut buf = BytesMut::with_capacity(65);

    // Packet len including length itself
    buf.put_u8((packet.as_ref().len() + 1) as u8);

    // Payload
    buf.extend_from_slice(packet.as_ref());

    // Checksum byte
    buf.put_u8(checksum(&buf));

    buf.freeze()
}

pub fn checksum<T: AsRef<[u8]>>(data: T) -> u8 {
    (data.as_ref().iter().map(|&x| x as u32).sum::<u32>() & 0xFF) as u8
}

pub fn unframe(response: Bytes) -> Result<Bytes, MiniDSPError> {
    let len = response[0] as usize;
    if response.len() < len {
        Err(MalformedResponse)
    } else {
        Ok(response.slice(1..len))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn frame_test() {
        let packet = Bytes::from_static(&[0x05, 0xFF, 0xDA, 0x02]);
        let framed = frame(packet.clone());

        assert_eq!(
            framed.len(),
            packet.len() + 2,
            "length should be len(data) + 1 (len) + 1 (checksum)"
        );
        assert_eq!(
            framed[0], 5,
            "the first byte should indicate the length of the packet including the checksum byte"
        );
        assert!(
            framed[1..5].iter().eq(packet.iter()),
            "the packet data should be there verbatim"
        );
        assert_eq!(framed[5], 229, "The checksum should be accurate");
    }

    #[test]
    fn unframe_test() {
        let response = Bytes::from_static(&[0x3, 0x1, 0x2, 0xFF, 0xFF, 0xFF]);
        let frame = unframe(response).unwrap();
        assert_eq!(
            frame,
            vec![0x1, 0x2],
            "should remove the length header and return the right data"
        );
    }
}
