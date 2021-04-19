//! Combines a Stream and Sink into a single object implementing both traits

use std::{pin::Pin, task::{Context, Poll}};

use futures::{Sink, Stream};
use pin_project::pin_project;

#[pin_project]
/// Object combining a stream and a sink
pub struct Combine<TSink, TStream> {
    #[pin]
    sink: TSink,
    #[pin]
    stream: TStream,
}

impl<'a, TSink, TStream> Combine<TSink, TStream> {
    pub fn new(stream: TStream, sink: TSink) -> Self {
        Self { sink, stream }
    }
}

impl<TSink, TStream> Stream for Combine<TSink, TStream>
where
    TStream: Stream,
{
    type Item = TStream::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }
}

impl<TSink, TStream, TSinkItem> Sink<TSinkItem> for Combine<TSink, TStream>
where
    TSink: Sink<TSinkItem>,
{
    type Error = TSink::Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.project().sink.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: TSinkItem) -> Result<(), Self::Error> {
        self.project().sink.start_send(item)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.project().sink.poll_flush(cx)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.project().sink.poll_close(cx)
    }
}
