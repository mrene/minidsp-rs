//! Multiplexes a command-response stream with an event stream holding unrelated device messages
//! Since the devices send unsolicited responses in the same stream, this remembers the last command
//! and matches it with the appropriate response. Unrelated received messages are pushed to another
//! channel.

use crate::{
    commands::{Commands, Responses},
    transport::MiniDSPError,
    Result,
};
use futures::{channel::oneshot, future::BoxFuture, Future, Sink, SinkExt, StreamExt};
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{self, Poll},
};
use task::Context;
use tokio::sync::broadcast;
use tower::Service;

type BoxSink<E> = Pin<Box<dyn Sink<Commands, Error = E> + Send + Sync>>;
type BoxStream = futures::stream::BoxStream<'static, Result<Responses, MiniDSPError>>;

type PendingCommandTuple = (Commands, oneshot::Sender<Result<Responses, MiniDSPError>>);

pub struct Multiplexer {
    /// If applicable, the last command that was sent, and a channel towards which the response
    /// should be sent.
    pending_command: Arc<Mutex<VecDeque<PendingCommandTuple>>>,

    /// The sending side of a broadcast channel used for received events
    event_tx: Arc<Mutex<Option<broadcast::Sender<Responses>>>>,

    /// Sink for sending commands
    write: tokio::sync::Mutex<BoxSink<MiniDSPError>>,
}

impl Multiplexer {
    pub fn new(rx: BoxStream, tx: BoxSink<MiniDSPError>) -> Arc<Self> {
        let (recv_send, _) = broadcast::channel::<Responses>(10);
        let transport = Arc::new(Self {
            pending_command: Arc::new(Mutex::new(VecDeque::new())),
            event_tx: Arc::new(Mutex::new(Some(recv_send.clone()))),
            write: tokio::sync::Mutex::new(tx),
        });

        // Spawn the receive task
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

    pub fn roundtrip(
        self: &Arc<Self>,
        cmd: Commands,
    ) -> impl Future<Output = Result<Responses, MiniDSPError>> {
        let this = self.clone();
        async move {
            let rx = {
                let (tx, rx) = oneshot::channel();
                let mut pending_command = this.pending_command.lock().unwrap();
                pending_command.push_back((cmd.clone(), tx));
                rx
            };

            let mut writer = this.write.lock().await;
            writer.send(cmd).await?;

            rx.await.map_err(|_| MiniDSPError::TransportClosed)?
        }
    }

    /// Receives responses from the transport, dispatches responses to the first pending command if it matches, else
    /// pushed it to the events broadcast.
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
                .ok_or(MiniDSPError::TransportClosed)??;

            log::trace!("recv: {:02x?}", data);

            {
                let mut pending_cmd = self.pending_command.lock().unwrap();
                let matches = if let Some((cmd, _)) = pending_cmd.front() {
                    cmd.matches_response(&data)
                } else {
                    false
                };

                if matches {
                    let (_, channel) = pending_cmd.pop_front().unwrap();
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

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Commands) -> Self::Future {
        Box::pin(self.roundtrip(req))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::commands::BytesWrap;
    use bytes::Bytes;
    use futures::channel::mpsc;
    use futures_util::join;

    #[tokio::test]
    async fn test_golden_path() {
        let (sink_tx, mut sink_rx) = mpsc::channel::<Commands>(10);
        let (mut stream_tx, stream_rx) = mpsc::channel::<Result<Responses, MiniDSPError>>(10);
        let sink_tx = sink_tx.sink_map_err(|_| MiniDSPError::TransportClosed);

        let mplex = Multiplexer::new(Box::pin(stream_rx), Box::pin(sink_tx));
        let resp1 = mplex.roundtrip(Commands::SetMute { value: true });
        let resp2 = mplex.roundtrip(Commands::ReadHardwareId);
        let answer = async move {
            let cmd = sink_rx.next().await.unwrap();
            assert!(matches!(cmd, Commands::SetMute { .. }));
            stream_tx.send(Ok(Responses::Ack)).await.unwrap();

            let cmd = sink_rx.next().await.unwrap();
            assert!(matches!(cmd, Commands::ReadHardwareId { .. }));
            stream_tx
                .send(Ok(Responses::HardwareId {
                    payload: BytesWrap(Bytes::from_static(b"allo")),
                }))
                .await
                .unwrap();
        };

        let (resp1, resp2, _) = join!(resp1, resp2, answer);
        assert!(matches!(resp1.unwrap(), Responses::Ack));
        assert!(matches!(resp2.unwrap(), Responses::HardwareId{..}));
    }
}
