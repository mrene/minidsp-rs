//! Multiplexes a command-response stream with an event stream holding unrelated device messages
//! Since the devices send unsolicited responses in the same stream, this remembers the last command
//! and matches it with the appropriate response. Unrelated received messages are pushed to another
//! channel.

use crate::commands::{Commands, Responses};
use crate::transport::MiniDSPError;
use futures::channel::oneshot;
use futures::task::{Context, Poll};
use futures::{Future, Sink, SinkExt, Stream, StreamExt};
use pin_project::{pin_project, pinned_drop};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, OwnedMutexGuard};

type BoxSink<E> = Pin<Box<dyn Sink<Commands, Error = E> + Send + Sync>>;
type BoxStream = Pin<Box<dyn Stream<Item = Responses> + Send>>;
type PendingCommandTuple = (Commands, oneshot::Sender<Result<Responses, MiniDSPError>>);

pub struct Multiplexer {
    /// If applicable, the last command that was sent, and a channel towards which the response
    /// should be sent.
    pending_command: Arc<Mutex<Option<PendingCommandTuple>>>,

    /// The sending side of a broadcast channel used for received events
    receiver_tx: Arc<Mutex<Option<broadcast::Sender<Responses>>>>,

    /// Inner struct wrapping the device handle, ensuring only one sender exists simultaneously
    /// The Arc is used to be able to hold a lock guard as 'static
    inner: Arc<tokio::sync::Mutex<Sender>>,
}

impl Multiplexer {
    pub fn new(rx: BoxStream, tx: BoxSink<anyhow::Error>) -> Arc<Self> {
        let (recv_send, _) = broadcast::channel::<Responses>(10);
        let transport = Arc::new(Self {
            pending_command: Arc::new(Mutex::new(None)),
            receiver_tx: Arc::new(Mutex::new(Some(recv_send.clone()))),
            inner: Arc::new(tokio::sync::Mutex::new(Sender::new(tx))),
        });

        transport
            .inner
            .try_lock()
            .unwrap()
            .transport
            .replace(transport.clone());

        {
            let transport = transport.clone();
            let receiver_tx = transport.receiver_tx.clone();
            tokio::spawn(async move {
                let _ = transport.recv_loop(recv_send, rx).await;
                let mut tx = receiver_tx.lock().unwrap();
                // Set `receiver_tx` to None to mark this as closed
                tx.take();
            });
        }
        transport
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

            {
                let mut pending_cmd = self.pending_command.lock().unwrap();
                let mut matches = false;

                if let Some((_cmd, _)) = pending_cmd.as_ref() {
                    // check if cmd matches
                    matches = true;
                }

                if matches {
                    let (_, channel) = pending_cmd.take().unwrap();
                    let _ = channel.send(Ok(data)); // Discard errors because it means the caller gave up
                    continue;
                }
            }

            log::trace!("recv: {:02x?}", data);

            // This response doesn't relate to a pending command
            sender
                .send(data)
                .map_err(|_| MiniDSPError::TransportClosed)?;
        }
    }

    pub fn subscribe(&self) -> Result<broadcast::Receiver<Responses>, MiniDSPError> {
        let receiver = self.receiver_tx.lock().unwrap();
        match receiver.as_ref() {
            Some(tx) => Ok(tx.subscribe()),
            None => Err(MiniDSPError::TransportClosed),
        }
    }

    pub async fn send_lock(&'_ self) -> OwnedMutexGuard<Sender> {
        self.inner.clone().lock_owned().await
    }
}

pub struct Sender {
    transport: Option<Arc<Multiplexer>>,
    write: BoxSink<anyhow::Error>,
}

impl Sender {
    fn new(device: BoxSink<anyhow::Error>) -> Self {
        Sender {
            write: device,
            transport: None,
        }
    }

    pub fn roundtrip(&mut self, cmd: Commands) -> PendingCommand {
        let mut pending_command = self
            .transport
            .as_ref()
            .unwrap()
            .pending_command
            .lock()
            .unwrap();

        let (tx, rx) = oneshot::channel();

        if pending_command.is_some() {
            // There is already an active command, this should not happen,
            tx.send(Err(MiniDSPError::ConcurencyError)).unwrap();
        } else {
            pending_command.replace((cmd.clone(), tx));
        }
        drop(pending_command);

        PendingCommand {
            sender: self,
            cmd: Some(cmd),
            channel: rx,
        }
    }

    pub async fn send(&mut self, cmd: Commands) -> Result<(), MiniDSPError> {
        log::trace!("send: {:02x?}", cmd);
        Ok(self
            .write
            .send(cmd)
            .await
            .map_err(|e| MiniDSPError::TransportFailure(e.to_string()))?)
    }
}

#[pin_project(PinnedDrop)]
pub struct PendingCommand<'sender> {
    sender: &'sender mut Sender,
    #[pin]
    channel: oneshot::Receiver<Result<Responses, MiniDSPError>>,
    cmd: Option<Commands>,
}

impl<'sender> Future for PendingCommand<'sender> {
    type Output = Result<Responses, MiniDSPError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(cmd) = self.cmd.take() {
            match self.sender.write.as_mut().poll_ready(cx)? {
                Poll::Ready(()) => self.sender.write.as_mut().start_send(cmd)?,
                Poll::Pending => {
                    self.cmd = Some(cmd);
                    return Poll::Pending;
                }
            }
        }

        // Flush the stream to make sure our command has been sent
        match self.sender.write.as_mut().poll_flush(cx) {
            Poll::Ready(x) => x?,
            Poll::Pending => return Poll::Pending,
        };

        // Poll the response channel until it's ready
        match self.project().channel.poll(cx) {
            Poll::Ready(out) => Poll::Ready(out.map_err(|e| MiniDSPError::TransportClosed)?),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[pinned_drop]
impl<'sender> PinnedDrop for PendingCommand<'sender> {
    fn drop(self: Pin<&mut Self>) {
        // Remove ourselves from the pending command list
        self.sender
            .transport
            .as_ref()
            .unwrap()
            .pending_command
            .lock()
            .unwrap()
            .take();
    }
}
