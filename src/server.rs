//! TCP server compatible with the official mobile and desktop application
use crate::{transport, MiniDSPError};
use anyhow::Result;
use bytes::Bytes;
use futures::{channel::mpsc, Sink, SinkExt, Stream, StreamExt};
use log::info;
use tokio::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    select,
    sync::broadcast,
};
use tokio_util::codec::Framed;

/// Forwards the given tcp stream to a transport.
/// This lets multiple users talk to the same device simultaneously, which depending on the
/// user could be problematic.
async fn forward(
    tcp: TcpStream,
    mut device_tx: mpsc::Sender<Bytes>,
    mut device_rx: broadcast::Receiver<Bytes>,
) -> Result<()> {
    // Apply framing to the TCP stream
    let mut remote = Framed::new(tcp, transport::net::Codec::new());

    loop {
        select! {
            frame = device_rx.recv() => {
                remote.send(frame?).await?;
            },
            frame = remote.next() => {
                device_tx.send(frame.ok_or(MiniDSPError::TransportClosed)??).await?;
            },
        }
    }
}

/// Listen and forward every incoming tcp connection to the given transport
pub async fn serve<A, T, E>(bind_address: A, transport: T) -> Result<()>
where
    A: ToSocketAddrs,
    T: Sink<Bytes, Error = E> + Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: Into<anyhow::Error>,
{
    // Setup a channel-based forwarder so we can multiplex multiple clients. This could do
    // command-level multiplexing eventually.

    let (sink, stream) = transport.split();
    let (sink_tx, mut sink_rx) = mpsc::channel::<Bytes>(10);
    let (stream_tx, mut stream_rx) = broadcast::channel::<Bytes>(10);

    // Receive
    {
        let stream_tx = stream_tx.clone();
        let mut stream = Box::pin(stream);
        tokio::spawn(async move {
            while let Some(Ok(frame)) = stream.next().await {
                stream_tx
                    .send(frame)
                    .map_err(|_| MiniDSPError::TransportClosed)?;
            }
            Err::<(), _>(MiniDSPError::TransportClosed)
        });
    }

    // Send
    {
        let mut sink = Box::pin(sink);
        tokio::spawn(async move {
            while let Some(frame) = sink_rx.next().await {
                sink.send(frame)
                    .await
                    .map_err(|_| MiniDSPError::TransportClosed)?;
            }
            Err::<(), _>(MiniDSPError::TransportClosed)
        });
    }

    let listener = TcpListener::bind(bind_address).await?;
    loop {
        select! {
           result = listener.accept() => {
                let (stream, addr) = result?;
                info!("New connection: {:?}", addr);

                let device_tx = sink_tx.clone();
                let device_rx = stream_tx.clone().subscribe();

                tokio::spawn(async move {
                    let result = forward(stream, device_tx, device_rx).await;

                    if let Err(e) = result {
                        log::error!("err: {:?}", e);
                    }

                    info!("Closed: {:?}", addr);
                });
           },
           result = stream_rx.recv() => {
                if result.is_err() {
                    return Err(MiniDSPError::TransportClosed.into())
                }
           }
        }
    }
}
