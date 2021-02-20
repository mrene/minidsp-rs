//! Combines a Stream and Sink into a single object implementing both traits

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
        Self { stream, sink }
    }
}

impl<TSink, TStream> Stream for Combine<TSink, TStream>
where
    TStream: Stream,
{
    type Item = TStream::Item;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }
}

impl<TSink, TStream, TSinkItem> Sink<TSinkItem> for Combine<TSink, TStream>
where
    TSink: Sink<TSinkItem>,
{
    type Error = TSink::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.project().sink.poll_ready(cx)
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: TSinkItem) -> Result<(), Self::Error> {
        self.project().sink.start_send(item)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.project().sink.poll_flush(cx)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.project().sink.poll_close(cx)
    }
}
