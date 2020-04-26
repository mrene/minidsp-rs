use hidapi::{HidDevice, HidError};

use crate::transport::Transport;

#[derive(Default)]
pub struct PacketFramer {
    pub omit_length: bool,
    pub omit_checksum: bool,
}

impl PacketFramer {
    pub fn new() -> Self {
        Self {
            omit_length: false,
            omit_checksum: false,
        }
    }
}

pub trait Framer: Send {
    fn frame(&self, packet: &[u8]) -> Vec<u8>;
    fn unframe(&self, response: &[u8]) -> Result<Vec<u8>, failure::Error>;
}

impl Framer for PacketFramer {
    // Formats an hid command
    fn frame(&self, packet: &[u8]) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::with_capacity(65);

        // HID report id 0
        buf.push(0);

        // Packet len including length itself
        if !self.omit_length {
            buf.push((packet.len() + 1) as u8);
        }

        // Payload
        buf.extend_from_slice(packet);

        // Add the checksum byte
        if !self.omit_checksum {
            buf.push((buf.iter().map(|&x| x as u32).sum::<u32>() & 0xFF) as u8);
        }

        // Pad with 0xFF
        buf.resize(65, 0xFF);
        buf
    }

    fn unframe(&self, response: &[u8]) -> Result<Vec<u8>, failure::Error> {
        let len = response[0] as usize;
        if response.len() < len {
            Err(failure::format_err!("response message was malformed"))
        } else if !self.omit_length {
            Ok(Vec::from(&response[1..len]))
        } else {
            Ok(Vec::from(&response[..len]))
        }
    }
}

pub struct HID<TFramer: Framer + Default = PacketFramer> {
    pub device: HidDevice,
    pub verbose: bool,
    pub log: bool,
    pub framer: TFramer,
}

impl<TFramer: Framer + Default> HID<TFramer> {
    pub fn new(device: HidDevice) -> Self {
        Self {
            device,
            verbose: false,
            log: false,
            framer: <TFramer>::default(),
        }
    }

    pub fn set_verbose(&mut self, value: bool) {
        self.verbose = value;
    }

    /// Drains the read buffers by waiting until a zero-sized non-blocking read
    pub fn drain(&mut self) -> Result<(), HidError> {
        self.device.set_blocking_mode(false)?;
        let mut buf = [0u8; 65];

        loop {
            let n = self.device.read(&mut buf)?;
            if n == 0 {
                break;
            }
            println!("*** missed roundtrip data: {:02x?}", &buf[..n]);
        }

        self.device.set_blocking_mode(true)?;

        Ok(())
    }
}

impl<TFramer: Framer + Default> Transport for HID<TFramer> {
    fn roundtrip(&mut self, packet: &[u8]) -> Result<Vec<u8>, failure::Error> {
        let buf = self.framer.frame(packet);

        self.drain()?;

        if self.verbose {
            println!("To device: {:02x?}", &buf);
        }
        self.device.write(&buf[..65])?;

        // if self.verbose {
        // eprintln!("written!");
        // }

        let mut read_buf = [0u8; 65];
        self.device.read(&mut read_buf)?;
        if self.verbose {
            println!("From device: {:02x?}", &read_buf[..]);
        }

        let resp = self.framer.unframe(&read_buf)?;
        if self.log {
            println!("Command: {:02x?} Response: {}", &buf, format_frame(&resp));
        }

        self.drain()?;

        Ok(resp)
    }
}

pub fn format_frame(frame: &[u8]) -> String {
    if frame.len() == 0 {
        return "".to_owned();
    }
    let framer = PacketFramer::new();
    match framer.unframe(frame) {
        Ok(f) => format!("{:02x?}", f),
        Err(e) => format!("failed to unframe [{:02x?}]: {:?}", frame, e),
    }
}

#[cfg(test)]
mod test {
    use std::iter;

    use super::*;

    #[test]
    fn frame_test() {
        let framer = PacketFramer::new();
        let packet = &[0x05, 0xFF, 0xDA, 0x02];
        let framed = framer.frame(packet);

        assert_eq!(framed.len(), 65, "packets should be 65 bytes long");
        assert_eq!(
            framed[0], 0,
            "the first byte should be 0x00 to indicate the hid report id"
        );
        assert_eq!(
            framed[1], 5,
            "the second byte should indicate the length of the packet including the checksum byte"
        );
        assert!(
            framed[2..6].iter().eq(packet.iter()),
            "the packet data should be there verbatim"
        );
        assert_eq!(framed[6], 229, "The checksum should be accurate");
        assert!(
            framed[7..].iter().eq(iter::repeat(&0xFFu8).take(58)),
            "the packet should be padded with 0xFF"
        );
    }

    #[test]
    fn unframe_test() {
        let response = &[0x3, 0x1, 0x2, 0xFF, 0xFF, 0xFF];
        let framer = PacketFramer::new();
        let frame = framer.unframe(response).unwrap();
        assert_eq!(
            frame,
            vec![0x1, 0x2],
            "should remove the length header and return the right data"
        );
    }
}
