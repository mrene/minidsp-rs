//! Allows talking with the [crate::server] component
mod codec;
mod discover;

use super::Transport;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;
use codec::Codec;

pub use discover::*;

pub struct StreamTransport {}

impl StreamTransport {
    pub fn new<A>(stream: A) -> Transport
    where
        A: AsyncRead + AsyncWrite + Send + 'static,
    {
        let (tx, rx) = Framed::new(stream, Codec::new()).split();
        let rx = rx.filter_map(|x| async move { x.ok() });
        Transport::new(Box::pin(rx), Box::pin(tx.sink_err_into()))
    }
}
