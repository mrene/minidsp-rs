use std::{fmt, io::Cursor};

use bytes::Bytes;
use futures::{channel::mpsc, SinkExt, Stream, StreamExt};
use tokio::{fs::File, io::AsyncRead};
use tokio_util::codec::{Decoder, LinesCodec};

use crate::{commands::Commands, packet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Sent(Bytes),
    Received(Bytes),
}

impl Message {
    pub fn from_string(s: &str) -> Option<Message> {
        let mut split = s.splitn(2, ": ");
        let prefix = split.next()?;
        match prefix {
            "Sent" => Some(Message::Sent(Message::parse_hex(split.next()?)?)),
            "Recv" => Some(Message::Received(Message::parse_hex(split.next()?)?)),
            _ => None,
        }
    }

    fn parse_hex(s: &str) -> Option<Bytes> {
        Some(Bytes::from(hex::decode(s).ok()?))
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::Sent(data) => {
                write!(f, "Sent: {}", hex::encode(data))
            }
            Message::Received(data) => {
                write!(f, "Recv: {}", hex::encode(data))
            }
        }
    }
}

/// Records commands, responses and events to a text file
pub struct Recorder {
    tx: mpsc::UnboundedSender<Message>,
}

impl Recorder {
    pub fn new(file: File) -> Self {
        let mut framed = LinesCodec::new().framed(file);
        let (tx, mut rx) = mpsc::unbounded::<Message>();

        tokio::spawn(async move {
            while let Some(msg) = rx.next().await {
                if framed.send(msg.to_string()).await.is_err() {
                    break;
                }
            }
        });

        Recorder { tx }
    }

    /// Feed a sent frame
    pub fn feed_sent(&mut self, frame: &Bytes) {
        let _ = self.tx.unbounded_send(Message::Sent(frame.clone()));
    }

    /// Feed a received frame
    pub fn feed_recv(&mut self, frame: &Bytes) {
        let _ = self.tx.unbounded_send(Message::Received(frame.clone()));
    }
}

pub fn from_reader<T: AsyncRead + Sized>(reader: T) -> impl Stream<Item = Message> {
    let framed = tokio_util::codec::FramedRead::new(reader, LinesCodec::new());
    framed.filter_map(|x| async { Message::from_string(x.ok()?.as_str()) })
}

pub fn fixtures_reader(data: &'static [u8]) -> impl Stream<Item = Message> {
    let r = Cursor::new(data);
    from_reader(r)
}

pub async fn decode_sent_commands(msg: Message) -> Option<Commands> {
    if let Message::Sent(data) = msg {
        let data = packet::unframe(data).ok()?;
        Some(Commands::from_bytes(data).ok()?)
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse() {
        let msg_str = "Sent: aabbccdd";
        let msg = Message::Sent(Bytes::from_static(&[0xaa, 0xbb, 0xcc, 0xdd]));
        assert!(msg.eq(&Message::from_string(msg_str).unwrap()));
        assert!(msg_str.eq(msg.to_string().as_str()));
    }

    #[tokio::test]
    async fn test_reader() {
        let data: &'static [u8] = include_bytes!("../../test_fixtures/config1/sync.txt");
        let mut x = Box::pin(fixtures_reader(data));
        while let Some(msg) = x.next().await {
            println!("{msg:02x?}");
        }
    }
}
