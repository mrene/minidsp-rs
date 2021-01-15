use bytes::{BufMut, Bytes, BytesMut};
use core::panic;
use futures::{Future, FutureExt, Sink, Stream};
use hidapi::{HidDevice, HidError};
use log::trace;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use super::wrapper::HidDeviceWrapper;

// 65 byte wide: 1 byte report id + 64 bytes data
const HID_PACKET_SIZE: usize = 65;

type SendFuture = Box<dyn Future<Output = Option<Bytes>> + Send>;
type RecvFuture = Box<dyn Future<Output = Result<(), HidError>> + Send>;

/// A stream of HID reports
pub struct HidStream {
    device: Arc<HidDeviceWrapper>,
    current_rx: Option<Pin<SendFuture>>,
    current_tx: Option<Pin<RecvFuture>>,
}

impl HidStream {
    pub fn new(device: HidDevice) -> Self {
        Self {
            current_rx: None,
            current_tx: None,
            device: Arc::new(HidDeviceWrapper::new(device)),
        }
    }

    fn recv(&self) -> impl Future<Output = Option<Bytes>> {
        let device = self.device.clone();
        async move {
            let mut read_buf = BytesMut::with_capacity(HID_PACKET_SIZE);
            read_buf.resize(HID_PACKET_SIZE, 0);

            loop {
                // Use a short timeout because we want to be able to bail out if the future gets dropped
                let size = tokio::task::block_in_place(|| device.read_timeout(&mut read_buf, 500));
                match size {
                    // read_timeout returns Ok(0) if a timeout has occurred
                    Ok(0) => continue,
                    Ok(size) => {
                        // successful read
                        read_buf.truncate(size);
                        trace!("read: {:02x?}", read_buf.as_ref());
                        return Some(read_buf.freeze());
                    }
                    Err(e) => {
                        // device error
                        log::error!("error in hid receive loop: {:?}", e);
                        return None;
                    }
                }
            }
        }
    }

    fn send(&self, frame: Bytes) -> impl Future<Output = Result<(), HidError>> {
        let mut buf = BytesMut::with_capacity(HID_PACKET_SIZE);

        // HID report id
        buf.put_u8(0);

        // Frame data
        buf.extend_from_slice(&frame);

        // Pad remaining packet data with 0xFF
        buf.resize(HID_PACKET_SIZE, 0xFF);

        let buf = buf.freeze();

        let device = self.device.clone();
        async move {
            tokio::task::block_in_place(|| device.write(&buf))?;
            Ok(())
        }
    }
}

impl Stream for HidStream {
    type Item = Bytes;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Create a read call if we need to
        let fut = {
            if self.current_rx.is_none() {
                self.current_rx = Some(Box::pin(self.recv().fuse()));
            }
            self.current_rx.as_mut().unwrap()
        };

        // Wait for a response
        let result = fut.as_mut().poll(cx);
        match result {
            Poll::Ready(data) => {
                // Data ready, clear the current future until the next call
                self.current_rx.take();
                Poll::Ready(data)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Sink<Bytes> for HidStream {
    type Error = HidError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // If we have a pending send future, poll it here
        if let Some(ref mut future) = self.current_tx {
            let result = future.as_mut().poll(cx);
            if result.is_ready() {
                self.current_tx.take();
            }
            return result;
        }

        // If not, we're ready to send
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        if self.current_tx.is_some() {
            panic!("start_send called without being ready")
        }

        // Start sending future
        self.current_tx = Some(Box::pin(self.send(item).fuse()));

        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_ready(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Poll for readiness to ensure that no sends are pending
        self.poll_ready(cx)
    }
}

#[cfg(test)]
mod test {
    use crate::transport::hid::initialize_api;

    use super::*;
    use futures::{SinkExt, StreamExt};

    #[tokio::test]
    #[ignore]
    async fn test_hid() {
        let api = initialize_api().unwrap();
        let device = api.open(0x2752, 0x0011).unwrap();
        let mut device = Box::pin(HidStream::new(device));
        device
            .send(Bytes::from_static(&[0x02, 0x31, 0x33]))
            .await
            .unwrap();
        let resp = device.next().await.unwrap();
        println!("{:02x?}", resp.as_ref());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn test_multi() {
        let api = initialize_api().unwrap();
        let device = api.open(0x2752, 0x0011).unwrap();
        let device = Box::new(HidStream::new(device));

        let (mut sink, mut stream) = device.split();
        tokio::spawn(async move {
            loop {
                sink.send(Bytes::from_static(&[0x02, 0x31, 0x33]))
                    .await
                    .unwrap();
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });

        tokio::spawn(async move {
            loop {
                let x = stream.next().await;
                println!("{:?}", x);
            }
        });
    }
}
