//! Allows talking with the [crate::server] component
use crate::discovery;
use crate::transport::{MiniDSPError, Openable, Sender, Transport};
use anyhow::Result;
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use log::debug;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};
use tokio_stream::StreamExt;

pub struct NetTransport {
    /// The sending side of a broadcast channel used for received messages
    receiver_tx: broadcast::Sender<Bytes>,

    /// Inner struct wrapping the device handle, ensuring only one sender exists simultaneously
    /// The Arc is used to be able to hold a lock guard as 'static
    inner: Arc<Mutex<Inner>>,
}

impl NetTransport {
    pub fn new(stream: TcpStream) -> Self {
        let (recv_send, _) = broadcast::channel::<Bytes>(10);
        let (rx, tx) = stream.into_split();
        tokio::spawn(NetTransport::recv_loop(recv_send.clone(), rx));

        NetTransport {
            receiver_tx: recv_send,
            inner: Arc::new(Mutex::new(Inner::new(tx))),
        }
    }

    async fn recv_loop(sender: broadcast::Sender<Bytes>, mut stream: OwnedReadHalf) -> Result<()> {
        loop {
            let mut read_buf = BytesMut::with_capacity(64);
            stream.read_buf(&mut read_buf).await?;
            debug!("read: {:02x?}", read_buf.as_ref());
            sender.send(read_buf.freeze())?;
        }
    }
}

#[async_trait]
impl Transport for NetTransport {
    fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.receiver_tx.subscribe()
    }

    async fn send_lock(&'_ self) -> Box<dyn Sender> {
        return Box::new(self.inner.clone().lock_owned().await);
    }
}

pub struct Inner {
    write: OwnedWriteHalf,
}

impl Inner {
    fn new(device: OwnedWriteHalf) -> Self {
        Inner { write: device }
    }
}

#[async_trait]
impl Sender for Inner {
    async fn send(&mut self, frame: Bytes) -> Result<(), MiniDSPError> {
        debug!("write: {:02x?}", frame.as_ref());
        Ok(self.write.write_all(frame.as_ref()).await?)
    }
}

pub struct Device {
    pub packet: discovery::DiscoveryPacket,
    pub ip: SocketAddr,
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:5333 {}", self.ip.ip(), self.packet.hostname)
    }
}

#[async_trait]
impl Openable for Device {
    type Transport = NetTransport;

    async fn open(&self) -> Result<Self::Transport, MiniDSPError> {
        Ok(NetTransport::new(
            TcpStream::connect(SocketAddr::new(self.ip.ip(), 5333)).await?,
        ))
    }
}

/// Gather discovery packets during the timeout period and return a de-duplicated list by ip
pub async fn discover(timeout: Duration) -> Result<Vec<Device>, anyhow::Error> {
    let mut devices = Box::new(HashMap::new());
    let mut stream = discovery::client::discover().await?;

    let timeout = tokio::time::sleep(timeout);
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some(Ok((packet, ip))) = stream.next() => {
                devices.insert(ip, Device { packet, ip });
            },
            _ = &mut timeout => break,
        }
    }

    Ok(devices.drain().map(|(_, v)| v).collect())
}
