use bytes::Bytes;
use futures::SinkExt;
use std::fmt;
use std::fmt::Formatter;
use tokio::fs::File;
use tokio::sync::mpsc;
use tokio_util::codec::{Decoder, LinesCodec};

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
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
        // assert!;
        let mut framed = LinesCodec::new().framed(file);
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if framed.send(msg.to_string()).await.is_err() {
                    break;
                }
            }
        });

        Recorder { tx }
    }

    /// Feed a sent frame
    pub fn feed_sent(&mut self, frame: &Bytes) {
        let _ = self.tx.send(Message::Sent(frame.clone()));
    }

    /// Feed a received frame
    pub fn feed_recv(&mut self, frame: &Bytes) {
        let _ = self.tx.send(Message::Received(frame.clone()));
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
}
