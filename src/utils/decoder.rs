//! Protocol decoder aiming at inspecting the app's interaction with the device
//! Frames are fed without the hid message id, but with the length prefix and crc value
//!
//! Once a frame is decoded, it's printed to the given writer

use std::fmt;

use bytes::Bytes;
use termcolor::{Color, ColorSpec, WriteColor};

use crate::{commands, packet};

/// Main decoder
pub struct Decoder {
    pub quiet: bool,
    pub w: Box<dyn WriteColor + Send + Sync>,
}

impl Decoder {
    /// Feed a sent frame
    pub fn feed_sent(&mut self, frame: &Bytes) {
        if let Ok(frame) = packet::unframe(frame.clone()) {
            let _ = self.print_frame(true, &frame);
            match commands::Commands::from_bytes(frame) {
                Ok(cmd) => {
                    if let commands::Commands::ReadFloats { .. } = cmd {
                        if self.quiet {
                            return;
                        }
                    }
                    let _ = self.print_command(cmd);
                }
                Err(err) => {
                    let _ = self.print_error(err);
                }
            };
        }
    }

    /// Feed a received frame
    pub fn feed_recv(&mut self, frame: &Bytes) {
        if let Ok(frame) = packet::unframe(frame.clone()) {
            let _ = self.print_frame(false, &frame);
            match commands::Responses::from_bytes(frame) {
                Ok(cmd) => {
                    if let commands::Responses::FloatData(_) = cmd {
                        if self.quiet {
                            return;
                        }
                    }
                    let _ = self.print_response(cmd);
                }
                Err(err) => {
                    let _ = self.print_error(err);
                }
            }
        }
    }

    fn print_frame(&mut self, sent: bool, frame: &Bytes) -> std::io::Result<()> {
        if self.quiet {
            return Ok(());
        }
        let _ = self.print_direction(sent);
        let _ = self
            .w
            .set_color(ColorSpec::new().set_fg(Some(Color::White)).set_dimmed(true));
        writeln!(self.w, "{:02x?}", frame.as_ref())?;

        Ok(())
    }

    fn print_command<T: fmt::Debug>(&mut self, cmd: T) -> std::io::Result<()> {
        let _ = self.print_direction(true);
        let _ = self.w.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
        writeln!(self.w, "{:02x?}", cmd)?;
        Ok(())
    }

    fn print_response<T: fmt::Debug>(&mut self, cmd: T) -> std::io::Result<()> {
        let _ = self.print_direction(false);
        let _ = self
            .w
            .set_color(ColorSpec::new().set_fg(Some(Color::Green)));
        writeln!(self.w, "{:02x?}", cmd)?;
        Ok(())
    }

    fn print_direction(&mut self, sent: bool) -> std::io::Result<()> {
        let direction = if sent {
            let _ = self.w.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
            "Sent: "
        } else {
            let _ = self.w.set_color(ColorSpec::new().set_fg(Some(Color::Blue)));
            "Recv: "
        };
        write!(self.w, "{}", direction)?;

        Ok(())
    }

    fn print_error<T: fmt::Debug>(&mut self, err: T) -> std::io::Result<()> {
        let _ = self.w.set_color(ColorSpec::new().set_fg(Some(Color::Red)));
        writeln!(self.w, "{:?}", err)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use termcolor::{ColorChoice, StandardStream};

    use super::*;
    #[test]
    fn test_print() {
        let writer = Box::new(StandardStream::stderr(ColorChoice::Always));
        let mut d = Decoder {
            w: writer,
            quiet: false,
        };
        d.feed_sent(&Bytes::from_static(&[0x05, 0x14, 0x00, 0x46, 0x04, 0x63]));
        d.feed_recv(&Bytes::from_static(&[
            0x05, 0x14, 0x00, 0x46, 0x00, 0x00, 0x00,
        ]));
        d.w.reset().unwrap();
    }
}
