use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::{channel::mpsc, Sink, Stream};
use futures_util::ready;
use pin_project::pin_project;

pub fn logger<T, TSent, TReceived>(
    inner: T,
    tx: mpsc::UnboundedSender<Message<TSent, TReceived>>,
) -> Logger<T, TSent, TReceived>
where
    TSent: Clone,
    TReceived: Clone,
{
    Logger { inner, tx }
}

/// Wraps a sink and/or stream and logs every sent/received elements
#[pin_project]
pub struct Logger<T, TSent, TReceived>
where
    TSent: Clone,
    TReceived: Clone,
{
    #[pin]
    inner: T,

    tx: mpsc::UnboundedSender<Message<TSent, TReceived>>,
}

pub enum Message<TSent, TReceived> {
    Sent(TSent),
    Received(TReceived),
}

impl<T, TSent, TReceived> Logger<T, TSent, TReceived>
where
    TSent: Clone,
    TReceived: Clone,
    T: Sink<TSent>,
    T: Stream,
{
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T, TSent, TReceived, TErr> Stream for Logger<T, TSent, TReceived>
where
    TSent: Clone,
    TReceived: Clone,
    T: Stream<Item = Result<TReceived, TErr>>,
{
    type Item = Result<TReceived, TErr>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let element = ready!(this.inner.poll_next(cx));
        if let Some(Ok(element)) = &element {
            let _ = this.tx.unbounded_send(Message::Received(element.clone()));
        }
        Poll::Ready(element)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T, TSent, TReceived> Sink<TSent> for Logger<T, TSent, TReceived>
where
    TSent: Clone,
    TReceived: Clone,
    T: Sink<TSent>,
{
    type Error = T::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: TSent) -> Result<(), Self::Error> {
        let this = self.project();
        this.tx.unbounded_send(Message::Sent(item.clone())).unwrap();
        this.inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}
