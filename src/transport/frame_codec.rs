//! Serializes and parses commands / responses while still maintaining a single Stream+Sink object

use std::task;

use crate::{
    commands::{self, Responses},
    packet,
};
use bytes::Bytes;
use futures::{Sink, Stream};
use pin_project::pin_project;
use task::Poll;

use super::MiniDSPError;

#[pin_project]
pub struct FrameCodec<Backend> {
    #[pin]
    inner: Backend,
}

fn parse_response(frame: Bytes) -> Result<Responses, MiniDSPError> {
    let packet = packet::unframe(frame)?;
    let response = Responses::from_bytes(packet.clone())?;
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

impl<Backend> Stream for FrameCodec<Backend>
where
    Backend: Stream<Item = Bytes>,
{
    type Item = Result<Responses, MiniDSPError>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.project().inner.poll_next(cx) {
            Poll::Ready(Some(frame)) => Poll::Ready(Some(parse_response(frame))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
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
        self.project().inner.start_send(item.to_bytes())
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
