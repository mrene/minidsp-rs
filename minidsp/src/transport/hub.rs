//! Provides a way to share a transport on a frame level, all received frames are forward to all clients.

use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use bytes::Bytes;
use futures::{channel::mpsc, Sink, SinkExt, Stream, StreamExt};
use futures_util::ready;
use pin_project::pin_project;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use super::{MiniDSPError, Transport};
use crate::utils::OwnedJoinHandle;

const CAPACITY: usize = 100;

/// Clonable transport which implements frame level forwarding
#[pin_project]
pub struct Hub {
    // Shared data between clients
    inner: Arc<Mutex<Option<Inner>>>,

    #[pin]
    device_rx: BroadcastStream<Bytes>,
    #[pin]
    device_tx: mpsc::Sender<Bytes>,

    // Join handle containing the wrapped transport
    read_handle: Arc<OwnedJoinHandle<()>>,
    send_handle: Arc<OwnedJoinHandle<()>>,
}

impl Hub {
    pub fn new(transport: Transport) -> Self {
        let (read_tx, read_rx) = broadcast::channel::<Bytes>(CAPACITY);
        let (send_tx, mut send_rx) = mpsc::channel(CAPACITY);
        let (mut device_tx, mut device_rx) = transport.split();
        let inner = Arc::new(Mutex::new(Some(Inner::new(read_tx))));

        let read_handle = {
            let inner = inner.clone();
            let read_tx = {
                let inner = inner.lock().unwrap();
                inner.as_ref().unwrap().device_rx.clone()
            };

            OwnedJoinHandle::new(tokio::spawn(async move {
                while let Some(frame) = device_rx.next().await {
                    match frame {
                        Ok(frame) => {
                            if let Err(e) = read_tx.send(frame) {
                                // receiver is gone
                                log::error!("read_tx receiver is gone: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            // The upstream transport reported an error
                            log::error!("recv error {}", e);
                            break;
                        }
                    }
                }
                inner.lock().unwrap().take();
            }))
        };

        let send_handle = {
            OwnedJoinHandle::new(tokio::spawn({
                let inner = inner.clone();
                async move {
                    while let Some(frame) = send_rx.next().await {
                        let res = device_tx.send(frame).await;
                        if let Err(e) = res {
                            log::error!("device_tx: {}", e);
                            break;
                        }
                    }
                    inner.lock().unwrap().take();
                }
            }))
        };

        Self {
            inner,
            device_rx: BroadcastStream::new(read_rx),
            device_tx: send_tx,

            send_handle: Arc::new(send_handle),
            read_handle: Arc::new(read_handle),
        }
    }

    /// Clones the transport if it is still available, returns None if it has been closed
    pub fn try_clone(&self) -> Option<Self> {
        let inner = self.inner.lock().unwrap();
        let device_rx = BroadcastStream::new(inner.as_ref()?.device_rx.subscribe());
        Some(Self {
            inner: self.inner.clone(),
            device_rx,
            device_tx: self.device_tx.clone(),
            send_handle: self.send_handle.clone(),
            read_handle: self.read_handle.clone(),
        })
    }
}

impl Stream for Hub {
    type Item = Result<Bytes, MiniDSPError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut device_rx: Pin<&mut BroadcastStream<_>> = self.project().device_rx;
        loop {
            let res = ready!(device_rx.as_mut().poll_next(cx));
            return Poll::Ready(match res {
                Some(Ok(obj)) => Some(Ok(obj)),
                Some(Err(e)) => {
                    log::warn!("lost messages: {:?}", e);
                    continue;
                }
                None => None,
            });
        }
    }
}

impl Sink<Bytes> for Hub {
    type Error = MiniDSPError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .device_tx
            .poll_ready(cx)
            .map_err(|_| MiniDSPError::TransportClosed)
    }

    fn start_send(self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        self.project()
            .device_tx
            .start_send(item)
            .map_err(|_| MiniDSPError::TransportClosed)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .device_tx
            .poll_flush(cx)
            .map_err(|_| MiniDSPError::TransportClosed)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .device_tx
            .poll_close(cx)
            .map_err(|_| MiniDSPError::TransportClosed)
    }
}

struct Inner {
    // Broadcast sender used for creating receivers through .subscribe()
    device_rx: broadcast::Sender<Bytes>,
}

impl Inner {
    pub fn new(device_rx: broadcast::Sender<Bytes>) -> Self {
        Self { device_rx }
    }
}
