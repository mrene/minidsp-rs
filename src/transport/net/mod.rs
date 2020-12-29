use crate::transport::{MiniDSPError, Sender, Transport};
use anyhow::Result;
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use log::debug;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};

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
