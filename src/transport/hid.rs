use hidapi::{HidDevice, HidError};
use crate::transport::Transport;

// Formats an hid command
fn frame_packet(packet: &[u8]) -> Vec<u8> {

    let mut buf: Vec<u8> = Vec::with_capacity(65);

    // HID report id 0
    buf.push(0);

    // Packet len including length itself
    buf.push((packet.len() + 1) as u8);

    // Payload
    buf.extend_from_slice(packet);

    // Add the checksum byte
    buf.push((buf.iter()
        .map(|&x| x as u32)
        .sum::<u32>() & 0xFF) as u8);

    // Pad with 0xFF
    buf.resize(65, 0xFF);
    buf
}

fn unframe_response(response: &[u8]) -> Vec<u8> {
    Vec::from(&response[1..(response[0] as usize)])
}

pub struct HID {
    device: HidDevice,
}

impl HID {
    pub fn new(device: HidDevice) -> Self {
        return Self { device };
    }

}

impl Transport for HID {
    type Error = HidError;

    fn roundtrip(&self, packet: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let buf = frame_packet(packet);
        self.device.write(&buf)?;

        let mut buf = [0u8; 64];
        self.device.read(&mut buf)?;

        Ok(unframe_response(&buf))
    }

}


#[cfg(test)]
mod test {
    use super::*;
    use std::iter;

    #[test]
    fn frame_test() {
        let packet = &[0x05, 0xFF, 0xDA, 0x02];
        let framed = frame_packet(packet);

        assert_eq!(framed.len(), 65, "packets should be 65 bytes long");
        assert_eq!(framed[0], 0, "the first byte should be 0x00 to indicate the hid report id");
        assert_eq!(framed[1], 5, "the second byte should indicate the length of the packet including the checksum byte");
        assert!(framed[2..6].iter().eq(packet.iter()), "the packet data should be there verbatim");
        assert_eq!(framed[6], 229, "The checksum should be accurate");
        assert!(framed[7..].iter().eq(iter::repeat(&0xFFu8).take(58)), "the packet should be padded with 0xFF");
    }

    #[test]
    fn unframe_test() {
        let response = &[0x3, 0x1, 0x2, 0xFF, 0xFF, 0xFF];
        let frame = unframe_response(response);
        assert_eq!(frame, vec![0x1, 0x2], "should remove the length header and return the right data");
    }
}

