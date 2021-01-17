//! Allows talking with the [crate::server] component
mod codec;
mod discover;

use std::sync::Arc;

use super::{frame_codec, multiplexer::Multiplexer};
pub(crate) use codec::Codec;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

pub use discover::*;

pub struct StreamTransport {}

impl StreamTransport {
    pub fn new<A>(stream: A) -> Arc<Multiplexer>
    where
        A: AsyncRead + AsyncWrite + Send + 'static,
    {
        let framed = Framed::new(stream, Codec::new());
        let framed = frame_codec::FrameCodec::new(framed);
        let (tx, rx) = framed.split();
        Multiplexer::new(Box::pin(rx), Box::pin(tx.sink_err_into()))
    }
}
