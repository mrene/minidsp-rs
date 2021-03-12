//! Provides a way to share a transport on a frame level, all received frames are forward to all clients.

use super::{MiniDSPError, Transport};
use crate::utils::OwnedJoinHandle;
use bytes::Bytes;
use futures::{channel::mpsc, Sink, SinkExt, Stream, StreamExt};
use futures_util::ready;
use pin_project::pin_project;
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

const CAPACITY: usize = 100;

/// Clonable transport which implements frame level forwarding
#[pin_project]
pub struct Hub {
    // Shared data between clients
    inner: Arc<Mutex<Inner>>,

    #[pin]
    device_rx: BroadcastStream<Bytes>,
    #[pin]
    device_tx: mpsc::Sender<Bytes>,
}

impl Hub {
    pub fn new(transport: Transport) -> Self {
        let (read_tx, read_rx) = broadcast::channel::<Bytes>(CAPACITY);
        let (send_tx, mut send_rx) = mpsc::channel(CAPACITY);
        let (mut device_tx, mut device_rx) = transport.split();

        let read_handle = {
            let read_tx = read_tx.clone();

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
            }))
        };

        let send_handle = OwnedJoinHandle::new(tokio::spawn({
            async move {
                while let Some(frame) = send_rx.next().await {
                    let res = device_tx.send(frame).await;
                    if let Err(e) = res {
                        log::error!("device_tx: {}", e);
                        break;
                    }
                }
            }
        }));

        // let send_handle = (send_handle);
        let inner = Inner::new(read_handle, send_handle, read_tx);
        Self {
            inner: Arc::new(Mutex::new(inner)),
            device_rx: BroadcastStream::new(read_rx),
            device_tx: send_tx,
        }
    }
}

impl Clone for Hub {
    fn clone(&self) -> Self {
        let inner = self.inner.lock().unwrap();
        let device_rx = BroadcastStream::new(inner.device_rx.subscribe());
        Self {
            inner: self.inner.clone(),
            device_rx,
            device_tx: self.device_tx.clone(),
        }
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
    // Join handle containing the wrapped transport
    #[allow(dead_code)]
    read_handle: OwnedJoinHandle<()>,
    #[allow(dead_code)]
    send_handle: OwnedJoinHandle<()>,

    // Broadcast sender used for creating receivers through .subscribe()
    device_rx: broadcast::Sender<Bytes>,
}

impl Inner {
    pub fn new(
        read_handle: OwnedJoinHandle<()>,
        send_handle: OwnedJoinHandle<()>,
        device_rx: broadcast::Sender<Bytes>,
    ) -> Self {
        Self {
            read_handle,
            send_handle,
            device_rx,
        }
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;
//     use crate::utils::Combine;

//     #[tokio::test]
//     async fn test() {
//         let (sink_tx, sink_rx) = mpsc::unbounded();
//         let (stream_tx, stream_rx) = mpsc::unbounded();
//         let transport = Combine::new(stream_rx, sink_tx);

//         let hub = Hub::new(Box::pin(transport));

//     }
// }
