//! Multiplexes a command-response stream with an event stream holding unrelated device messages
//! Since the devices send unsolicited responses in the same stream, this remembers the last command
//! and matches it with the appropriate response. Unrelated received messages are pushed to another
//! channel.

use crate::{
    commands::{Commands, Responses},
    config::RestoreBlob,
    transport::MiniDSPError,
    Result,
};
use futures::{channel::oneshot, future::BoxFuture, Sink, SinkExt, Stream, StreamExt};

use std::{pin::Pin, sync::{Arc, Mutex}, task::Poll};
use tokio::sync::broadcast;
use tower::Service;

type BoxSink<E> = Pin<Box<dyn Sink<Commands, Error = E> + Send + Sync>>;
type BoxStream = Pin<Box<dyn Stream<Item = Responses> + Send>>;
type PendingCommandTuple = (Commands, oneshot::Sender<Result<Responses, MiniDSPError>>);

pub struct Multiplexer {
    /// If applicable, the last command that was sent, and a channel towards which the response
    /// should be sent.
    pending_command: Arc<Mutex<Option<PendingCommandTuple>>>,

    /// The sending side of a broadcast channel used for received events
    event_tx: Arc<Mutex<Option<broadcast::Sender<Responses>>>>,

    // Inner struct wrapping the device handle, ensuring only one sender exists simultaneously
    // The Arc is used to be able to hold a lock guard as 'static
    // inner: Arc<tokio::sync::Mutex<Sender>>,
    write: tokio::sync::Mutex<BoxSink<anyhow::Error>>,
}

impl Multiplexer {
    pub fn new(rx: BoxStream, tx: BoxSink<anyhow::Error>) -> Arc<Self> {
        let (recv_send, _) = broadcast::channel::<Responses>(10);
        let transport = Arc::new(Self {
            pending_command: Arc::new(Mutex::new(None)),
            event_tx: Arc::new(Mutex::new(Some(recv_send.clone()))),
            write: tokio::sync::Mutex::new(tx),
        });

        {
            let transport = transport.clone();
            let receiver_tx = transport.event_tx.clone();
            tokio::spawn(async move {
                let _ = transport.recv_loop(recv_send, rx).await;
                let mut tx = receiver_tx.lock().unwrap();
                // Set `receiver_tx` to None to mark this as closed
                tx.take();
            });
        }
        transport
    }

    pub async fn roundtrip(self: Arc<Self>, cmd: Commands) -> Result<Responses, MiniDSPError> {
        let (tx, rx) = oneshot::channel();
        {
            let mut pending_command = self.pending_command.lock().unwrap();
            if pending_command.is_some() {
                // There is already an active command, this should not happen,
                tx.send(Err(MiniDSPError::ConcurencyError)).unwrap();
            } else {
                pending_command.replace((cmd.clone(), tx));
            }
            drop(pending_command);
        }

        self.write.lock().await.send(cmd).await?;
        rx.await.map_err(|_| MiniDSPError::TransportClosed)?
    }

    async fn recv_loop(
        self: Arc<Self>,
        sender: broadcast::Sender<Responses>,
        mut stream: BoxStream,
    ) -> Result<(), MiniDSPError> {
        loop {
            let data = stream
                .as_mut()
                .next()
                .await
                .ok_or(MiniDSPError::TransportClosed)?;

            log::trace!("recv: {:02x?}", data);

            {
                let mut pending_cmd = self.pending_command.lock().unwrap();
                let matches = if let Some((cmd, _)) = pending_cmd.as_ref() {
                    cmd.matches_response(&data)
                } else {
                    false
                };

                if matches {
                    let (_, channel) = pending_cmd.take().unwrap();
                    let _ = channel.send(Ok(data)); // Discard errors because it means the caller gave up
                    continue;
                }
            }

            // This response doesn't relate to a pending command
            sender
                .send(data)
                .map_err(|_| MiniDSPError::TransportClosed)?;
        }
    }

    /// Subscribes to events that aren't related to a command
    pub fn subscribe(&self) -> Result<broadcast::Receiver<Responses>, MiniDSPError> {
        let receiver = self.event_tx.lock().unwrap();
        match receiver.as_ref() {
            Some(tx) => Ok(tx.subscribe()),
            None => Err(MiniDSPError::TransportClosed),
        }
    }
}

impl Service<Commands> for Arc<Multiplexer> {
    type Response = Responses;
    type Error = MiniDSPError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Commands) -> Self::Future {
        Box::pin(self.clone().roundtrip(req))
    }
}
