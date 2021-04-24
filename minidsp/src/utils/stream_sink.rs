use futures::{Sink, Stream};

/// Trait automatically implemented for objects that both implement Stream and Sink
pub trait StreamSink<'a, StreamItem, SinkItem, SinkError>:
    Stream<Item = StreamItem> + Sink<SinkItem, Error = SinkError> + 'a
{
}

impl<'a, T, StreamItem, SinkItem, SinkError> StreamSink<'a, StreamItem, SinkItem, SinkError> for T
where
    T: Stream<Item = StreamItem> + 'a,
    T: Sink<SinkItem, Error = SinkError> + 'a,
{
}

#[cfg(test)]
mod test {
    use std::{
        pin::Pin,
        task::{Context, Poll},
    };

    use futures::{Sink, Stream};

    use super::StreamSink;

    struct FakeStreamSink {}

    impl Stream for FakeStreamSink {
        type Item = ();

        fn poll_next(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            Poll::Ready(Some(()))
        }
    }

    impl Sink<()> for FakeStreamSink {
        type Error = ();

        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, _: ()) -> Result<(), Self::Error> {
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[test]
    fn test_usage() {
        let fake = FakeStreamSink {};
        let _: &dyn StreamSink<(), (), ()> = &fake;
    }
}
