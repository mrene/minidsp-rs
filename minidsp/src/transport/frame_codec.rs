//! Serializes and parses commands / responses while still maintaining a single Stream+Sink object

use std::task;

use bytes::Bytes;
use futures::{Sink, Stream};
use futures_util::ready;
use pin_project::pin_project;
use task::Poll;

use super::MiniDSPError;
use crate::{
    commands::{self, Responses},
    packet,
    utils::ErrInto,
};

#[pin_project]
pub struct FrameCodec<Backend> {
    #[pin]
    inner: Backend,
}

fn parse_response(frame: Bytes) -> Result<Responses, MiniDSPError> {
    let packet = packet::unframe(frame)?;
    let response = Responses::from_bytes(packet)?;
    Ok(response)
}

impl<Backend> FrameCodec<Backend>
where
    Backend: Stream + Sink<Bytes>,
{
    pub fn new(inner: Backend) -> Self {
        FrameCodec { inner }
    }
}

impl<Backend, E> Stream for FrameCodec<Backend>
where
    Backend: Stream<Item = Result<Bytes, E>>,
    E: Into<MiniDSPError>,
{
    type Item = Result<Responses, MiniDSPError>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let frame = ready!(self.project().inner.poll_next(cx));
        Poll::Ready(match frame {
            Some(frame) => Some(parse_response(frame.err_into()?)),
            None => None,
        })
    }
}

impl<Backend> Sink<commands::Commands> for FrameCodec<Backend>
where
    Backend: Sink<Bytes>,
{
    type Error = Backend::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(
        self: std::pin::Pin<&mut Self>,
        item: commands::Commands,
    ) -> Result<(), Self::Error> {
        self.project()
            .inner
            .start_send(packet::frame(item.to_bytes()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}
