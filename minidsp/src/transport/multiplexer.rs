//! Multiplexes a command-response stream with an event stream holding unrelated device messages
//! Since the devices send unsolicited responses in the same stream, this remembers the last command
//! and matches it with the appropriate response. Unrelated received messages are pushed to another
//! channel.

use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use bytes::Bytes;
use futures::{
    channel::oneshot, future::BoxFuture, Future, Sink, SinkExt, StreamExt, TryStreamExt,
};
use tokio::sync::broadcast;
use tower::Service;

use super::frame_codec::FrameCodec;
use crate::{
    commands::{Commands, Responses},
    transport::MiniDSPError,
    utils::StreamSink,
    Result,
};

type BoxSink<E> = Pin<Box<dyn Sink<Commands, Error = E> + Send + Sync>>;
type BoxStream = futures::stream::BoxStream<'static, Result<Responses, MiniDSPError>>;
type PendingCommandTuple = (Commands, oneshot::Sender<Result<Responses, MiniDSPError>>);

/// A command-response multiplexer capable of handling multiple clients
/// This differs from a typical pipeline command multiplexer by the fact that it must handle
/// unsolicited event frames from the upstream device. Each command contains logic to determine its appropriate response
/// so that unrelated responses are ignored. This is required because the device sends events formatted the same way
/// as normal responses, without any tag associating commands to responses.
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
    pub fn new<S>(backend: S) -> Arc<Self>
    where
        S: StreamSink<'static, Result<Responses, MiniDSPError>, Commands, MiniDSPError> + Send,
    {
        let (tx, rx) = backend.split();
        Self::from_split(Box::pin(tx), Box::pin(rx))
    }

    pub fn from_split(tx: BoxSink<MiniDSPError>, rx: BoxStream) -> Arc<Self> {
        let (recv_send, _) = broadcast::channel::<Responses>(10);
        let transport = Arc::new(Self {
            pending_command: Arc::new(Mutex::new(VecDeque::new())),
            event_tx: Arc::new(Mutex::new(Some(recv_send.clone()))),
            write: tokio::sync::Mutex::new(tx),
        });

        // Spawn the receive task
        {
            let receiver_tx = transport.event_tx.clone();
            let pending_commands = transport.pending_command.clone();

            tokio::spawn(async move {
                let result = Multiplexer::recv_loop(pending_commands.clone(), recv_send, rx).await;
                if let Err(e) = result {
                    log::error!("recv loop exit: {e:?}");

                    // Clear any pending commands by propagating the error
                    let mut pending = pending_commands.lock().unwrap();
                    while let Some((_, tx)) = pending.pop_front() {
                        let _ = tx.send(Err(MiniDSPError::TransportClosed));
                    }
                } else {
                    log::warn!("recv loop exited without an error");
                }
                let mut tx = receiver_tx.lock().unwrap();
                // Set `receiver_tx` to None to mark this as closed
                tx.take();
            });
        }
        transport
    }

    pub fn from_transport<T>(transport: T) -> Arc<Self>
    where
        T: StreamSink<'static, Result<Bytes, MiniDSPError>, Bytes, MiniDSPError> + Send,
    {
        Multiplexer::new(FrameCodec::new(transport).sink_err_into().err_into())
    }

    pub fn roundtrip(
        self: &Arc<Self>,
        cmd: Commands,
    ) -> impl Future<Output = Result<Responses, MiniDSPError>> {
        let this = self.clone();
        async move {
            let rx = {
                // Hold the write lock while queuing the command, so the order of pending commands
                // always matches the order in which they are written to the device.
                let mut writer = this.write.lock().await;

                let (tx, rx) = oneshot::channel();
                this.pending_command
                    .lock()
                    .unwrap()
                    .push_back((cmd.clone(), tx));

                log::trace!("send: {:02x?}", &cmd);
                if let Err(e) = writer.send(cmd).await {
                    // The command never made it to the device, don't wait for its response
                    drop(rx);
                    Self::purge_cancelled(&mut this.pending_command.lock().unwrap());
                    return Err(e);
                }
                rx
            };

            rx.await.map_err(|_| MiniDSPError::TransportClosed)?
        }
    }

    /// Removes pending commands whose caller is no longer waiting for a response (e.g. because
    /// the request timed out). Leaving them in the queue would make them capture responses
    /// meant for subsequent commands.
    fn purge_cancelled(pending: &mut VecDeque<PendingCommandTuple>) {
        pending.retain(|(_, tx)| !tx.is_canceled());
    }

    /// Receives responses from the transport, dispatches responses to the first pending command if it matches, else
    /// pushes it to the events broadcast channel.
    async fn recv_loop(
        pending_command: Arc<Mutex<VecDeque<PendingCommandTuple>>>,
        sender: broadcast::Sender<Responses>,
        mut stream: BoxStream,
    ) -> Result<(), MiniDSPError> {
        loop {
            let data = stream
                .as_mut()
                .next()
                .await
                .ok_or(MiniDSPError::TransportClosed)??;

            log::trace!("recv: {data:02x?}");

            {
                let mut pending_cmd = pending_command.lock().unwrap();
                Self::purge_cancelled(&mut pending_cmd);
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

            // This response doesn't relate to a pending command. Forward it to the event channel,
            // and ignore any errors that would arise if there were no bound receivers.
            let _ = sender.send(data);
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

    /// Constructs a MultiplexerServive which implements the tower::Service trait
    pub fn to_service(self: Arc<Self>) -> MultiplexerService {
        MultiplexerService(self)
    }
}

// Arc<T> is not marked as #[fundamental], therefore we cannot directly implement Service on Arc<Multiplexer>
/// Wraps a Multiplexer object in a cloneable struct implementing tower::Service
pub struct MultiplexerService(pub Arc<Multiplexer>);

impl Service<Commands> for MultiplexerService {
    type Response = Responses;
    type Error = MiniDSPError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Commands) -> Self::Future {
        let this = self.0.clone();
        Box::pin(this.roundtrip(req))
    }
}

impl std::ops::Deref for MultiplexerService {
    type Target = Arc<Multiplexer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MultiplexerService {
    pub async fn shutdown(&self) {
        let mut write = self.write.lock().await;
        if let Err(e) = write.close().await {
            log::error!("error shutting down multiplexer service: {e}");
        }
    }
}

#[cfg(test)]
mod test {

    use futures::channel::mpsc;
    use minidsp_protocol::HardwareId;

    use super::*;

    #[tokio::test]
    async fn test_golden_path() {
        let (sink_tx, mut sink_rx) = mpsc::channel::<Commands>(10);
        let (mut stream_tx, stream_rx) = mpsc::channel::<Result<Responses, MiniDSPError>>(10);
        let sink_tx = sink_tx.sink_map_err(|_| MiniDSPError::TransportClosed);

        let mplex = Multiplexer::from_split(Box::pin(sink_tx), Box::pin(stream_rx));
        let resp1 = mplex.roundtrip(Commands::SetMute { value: true });
        let resp2 = mplex.roundtrip(Commands::ReadHardwareId);
        let answer = async move {
            let cmd = sink_rx.next().await.unwrap();
            assert!(matches!(cmd, Commands::SetMute { .. }));
            stream_tx.send(Ok(Responses::Ack)).await.unwrap();

            let cmd = sink_rx.next().await.unwrap();
            assert!(matches!(cmd, Commands::ReadHardwareId));
            stream_tx
                .send(Ok(Responses::HardwareId(HardwareId {
                    fw_major: 1,
                    fw_minor: 10,
                    hw_id: 10,
                })))
                .await
                .unwrap();
        };

        let (resp1, resp2, _) = futures_util::join!(resp1, resp2, answer);
        assert!(matches!(resp1.unwrap(), Responses::Ack));
        assert!(matches!(resp2.unwrap(), Responses::HardwareId { .. }));
    }

    #[tokio::test]
    async fn test_timed_out_command_does_not_block_queue() {
        use std::time::Duration;

        let (sink_tx, mut sink_rx) = mpsc::channel::<Commands>(10);
        let (mut stream_tx, stream_rx) = mpsc::channel::<Result<Responses, MiniDSPError>>(10);
        let sink_tx = sink_tx.sink_map_err(|_| MiniDSPError::TransportClosed);
        let mplex = Multiplexer::from_split(Box::pin(sink_tx), Box::pin(stream_rx));

        // The device never answers this one, and the caller gives up
        let timed_out = tokio::time::timeout(
            Duration::from_millis(50),
            mplex.roundtrip(Commands::ReadHardwareId),
        )
        .await;
        assert!(timed_out.is_err());
        assert!(matches!(
            sink_rx.next().await,
            Some(Commands::ReadHardwareId)
        ));

        // The next command must still receive its own response
        let resp = mplex.roundtrip(Commands::SetMute { value: true });
        let answer = async move {
            let cmd = sink_rx.next().await.unwrap();
            assert!(matches!(cmd, Commands::SetMute { .. }));
            stream_tx.send(Ok(Responses::Ack)).await.unwrap();
        };
        let (resp, _) =
            futures_util::join!(tokio::time::timeout(Duration::from_secs(1), resp), answer);
        assert!(matches!(resp, Ok(Ok(Responses::Ack))));
    }
}
