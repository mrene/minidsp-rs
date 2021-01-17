//! Allows talking with the [crate::server] component
mod codec;
mod discover;

use std::sync::Arc;

use super::{frame_codec, multiplexer::Multiplexer, IntoTransport};
pub(crate) use codec::Codec;
use futures::{SinkExt, StreamExt, TryStreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

pub use discover::*;

pub struct StreamTransport<T>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
{
    framed: Framed<T, Codec>,
}

impl<T> StreamTransport<T>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
{
    pub fn new(stream: T) -> StreamTransport<T> {
        StreamTransport {
            framed: Framed::new(stream, Codec::new()),
        }
    }

    pub fn into_multiplexer(self) -> Arc<Multiplexer> {
        let framed = frame_codec::FrameCodec::new(self.framed);
        let (tx, rx) = framed.split();
        Multiplexer::new(Box::pin(rx), Box::pin(tx.sink_err_into()))
    }

    pub fn into_inner(self) -> Framed<T, Codec> {
        self.framed
    }
}

impl<T> IntoTransport for StreamTransport<T>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
{
    fn into_transport(self) -> super::Transport {
        Box::pin(self.into_inner().sink_err_into().err_into())
    }
}
