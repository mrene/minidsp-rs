//! Allows talking with the [crate::server] component
mod codec;
mod discover;

use std::sync::Arc;

use super::{frame_codec::FrameCodec, multiplexer::Multiplexer, IntoTransport, MiniDSPError};
pub use codec::Codec;
pub use discover::{discover, discover_timeout};
use futures::{SinkExt, TryStreamExt};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_util::codec::Framed;
use url2::Url2;

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
            framed: Framed::new(stream, Codec::new_client()),
        }
    }

    pub fn into_multiplexer(self) -> Arc<Multiplexer> {
        Multiplexer::new(FrameCodec::new(self.framed).sink_err_into().err_into())
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

pub async fn open_url(url: Url2) -> Result<StreamTransport<TcpStream>, MiniDSPError> {
    let host = url.host().ok_or(MiniDSPError::InvalidURL)?;
    let port = url.port().unwrap_or(5333);

    let stream = TcpStream::connect(format!("{}:{}", host, port)).await?;
    Ok(StreamTransport::new(stream))
}
