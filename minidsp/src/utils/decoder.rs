//! Protocol decoder aiming at inspecting the app's interaction with the device
//! Frames are fed without the hid message id, but with the length prefix and crc value
//!
//! Once a frame is decoded, it's printed to the given writer

use std::fmt;

use bimap::BiMap;
use bytes::Bytes;
use termcolor::{Color, ColorSpec, WriteColor};

use crate::{
    commands,
    commands::{Commands, Responses},
    packet,
};

/// Main decoder
pub struct Decoder {
    quiet: bool,
    w: Box<dyn WriteColor + Send + Sync>,
    name_map: Option<BiMap<String, usize>>,
    start_instant: std::time::Instant,
}

impl Decoder {
    pub fn new(
        w: Box<dyn WriteColor + Send + Sync>,
        quiet: bool,
        name_map: Option<BiMap<String, usize>>,
    ) -> Self {
        Decoder {
            quiet,
            w,
            name_map,
            start_instant: std::time::Instant::now(),
        }
    }

    /// Sets the symbol names to be printed
    pub fn set_name_map<'a>(&mut self, it: impl Iterator<Item = (&'a str, u16)>) {
        let mut map = BiMap::new();
        for (k, v) in it {
            map.insert(k.to_string(), v as usize);
        }
        self.name_map.replace(map);
    }

    /// Feed a sent frame
    pub fn feed_sent(&mut self, frame: &Bytes) {
        if let Ok(frame) = packet::unframe(frame.clone()) {
            match commands::Commands::from_bytes(frame.clone()) {
                Ok(cmd) => {
                    if matches!(
                        cmd,
                        commands::Commands::ReadFloats { .. } | commands::Commands::Read { .. }
                    ) && self.quiet
                    {
                        return;
                    }
                    let _ = self.print_frame(true, &frame);
                    let _ = self.print_command(cmd);
                }
                Err(err) => {
                    let _ = self.print_frame(true, &frame);
                    let _ = self.print_error(err);
                }
            };
        }
    }

    /// Feed a received frame
    pub fn feed_recv(&mut self, frame: &Bytes) {
        if let Ok(frame) = packet::unframe(frame.clone()) {
            match commands::Responses::from_bytes(frame.clone()) {
                Ok(cmd) => {
                    if matches!(
                        cmd,
                        commands::Responses::FloatData(_) | commands::Responses::Read { .. }
                    ) && self.quiet
                    {
                        return;
                    }
                    let _ = self.print_frame(false, &frame);
                    let _ = self.print_response(cmd);
                }
                Err(err) => {
                    let _ = self.print_frame(false, &frame);
                    let _ = self.print_error(err);
                }
            }
        }
    }

    fn print_time(&mut self) -> std::io::Result<()> {
        let elapsed = self.start_instant.elapsed();
        let secs = elapsed.as_secs();
        let millis = elapsed.subsec_millis();
        let _ = write!(self.w, "[{secs}.{millis:03}s] ");
        Ok(())
    }

    fn print_frame(&mut self, sent: bool, frame: &Bytes) -> std::io::Result<()> {
        let _ = self.print_direction(sent);
        let _ = self
            .w
            .set_color(ColorSpec::new().set_fg(Some(Color::White)).set_dimmed(true));
        writeln!(self.w, "{:02x?}", frame.as_ref())?;

        Ok(())
    }

    fn print_command(&mut self, cmd: Commands) -> std::io::Result<()> {
        let _ = self.print_direction(true);
        let _ = self.w.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
        write!(self.w, "{cmd:02x?} ")?;
        let _ = self.maybe_print_addr(&ParsedMessage::Request(cmd));
        Ok(())
    }

    fn print_response(&mut self, cmd: Responses) -> std::io::Result<()> {
        let _ = self.print_direction(false);
        let _ = self
            .w
            .set_color(ColorSpec::new().set_fg(Some(Color::Green)));
        write!(self.w, "{cmd:02x?}")?;
        let _ = self.maybe_print_addr(&ParsedMessage::Response(cmd));
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
        let _ = self.print_time();
        write!(self.w, "{direction}")?;

        Ok(())
    }

    fn print_error<T: fmt::Debug>(&mut self, err: T) -> std::io::Result<()> {
        let _ = self.w.set_color(ColorSpec::new().set_fg(Some(Color::Red)));
        let _ = self.print_time();
        writeln!(self.w, "Decode error: {err:?}")?;
        Ok(())
    }

    fn maybe_print_addr(&mut self, cmd: &ParsedMessage) -> std::io::Result<()> {
        let addr = match *cmd {
            ParsedMessage::Request(Commands::ReadFloats { addr, .. }) => addr,
            ParsedMessage::Request(Commands::Write { addr, .. }) => addr.val as _,
            ParsedMessage::Request(Commands::WriteBiquad { addr, .. }) => addr.val as _,
            ParsedMessage::Request(Commands::WriteBiquadBypass { addr, .. }) => addr.val as _,
            ParsedMessage::Request(Commands::Read { addr, .. }) => addr.val as _,
            ParsedMessage::Request(Commands::SwitchMux { addr, .. }) => addr.val as _,
            _ => {
                return writeln!(self.w);
            }
        };

        let _ = self
            .w
            .set_color(ColorSpec::new().set_fg(Some(Color::Magenta)));

        let name = self
            .resolve_addr(addr)
            .unwrap_or_else(|| "<unknown>".to_string());
        writeln!(self.w, "(0x{addr:02x?} | {addr:?}) <> {name}",)?;
        Ok(())
    }

    fn resolve_addr(&self, addr: u16) -> Option<String> {
        Some(
            self.name_map
                .as_ref()?
                .get_by_right(&(addr as usize))?
                .clone(),
        )
    }
}

pub enum ParsedMessage {
    Request(Commands),
    Response(Responses),
}

#[cfg(test)]
mod test {
    use termcolor::{ColorChoice, StandardStream};

    use super::*;
    #[test]
    fn test_print() {
        let writer = Box::new(StandardStream::stderr(ColorChoice::Always));
        let mut d = Decoder::new(writer, false, None);
        d.feed_sent(&Bytes::from_static(&[0x05, 0x14, 0x00, 0x46, 0x04, 0x63]));
        d.feed_recv(&Bytes::from_static(&[
            0x05, 0x14, 0x00, 0x46, 0x00, 0x00, 0x00,
        ]));
        d.w.reset().unwrap();
    }
}
