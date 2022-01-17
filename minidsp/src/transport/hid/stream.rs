use core::panic;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    thread, time,
};

use bytes::{BufMut, Bytes, BytesMut};
use futures::{
    channel::{mpsc, mpsc::TrySendError},
    Future, FutureExt, Sink, Stream,
};
use hidapi::{HidDevice, HidError};
use pin_project::pin_project;

use super::wrapper::HidDeviceWrapper;

// 65 byte wide: 1 byte report id + 64 bytes data
const HID_PACKET_SIZE: usize = 65;

type SendFuture = Box<dyn Future<Output = Result<(), HidError>> + Send>;

/// A stream of HID reports
#[pin_project]
pub struct HidStream {
    device: Arc<HidDeviceWrapper>,

    #[pin]
    rx: mpsc::UnboundedReceiver<Result<Bytes, HidError>>,

    current_tx: Option<Pin<SendFuture>>,
}

impl HidStream {
    pub fn new(device: HidDevice) -> Self {
        let device = Arc::new(HidDeviceWrapper::new(device));
        let (tx, rx) = mpsc::unbounded();
        Self::start_recv_loop(device.clone(), tx);

        Self {
            rx,
            current_tx: None,
            device,
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
            tokio::task::block_in_place(|| {
                let mut remaining_tries = 10;
                let mut result = device.write(&buf);
                while let Err(e) = result {
                    log::warn!("retrying usb write: {:?}", e);
                    thread::sleep(time::Duration::from_millis(250));
                    result = device.write(&buf);
                    remaining_tries -= 1;
                    if remaining_tries == 0 {
                        return result;
                    }
                }
                result
            })?;
            Ok(())
        }
    }

    fn start_recv_loop(
        device: Arc<HidDeviceWrapper>,
        tx: mpsc::UnboundedSender<Result<Bytes, HidError>>,
    ) {
        thread::spawn(move || {
            loop {
                if tx.is_closed() {
                    return Ok::<(), TrySendError<_>>(());
                }

                let mut read_buf = BytesMut::with_capacity(HID_PACKET_SIZE);
                read_buf.resize(HID_PACKET_SIZE, 0);

                // Use a short timeout because we want to be able to bail out if the receiver gets
                // dropped.
                let size = device.read_timeout(&mut read_buf, 500);
                match size {
                    // read_timeout returns Ok(0) if a timeout has occurred
                    Ok(0) => continue,
                    Ok(size) => {
                        // successful read
                        read_buf.truncate(size);
                        log::trace!("read: {:02x?}", read_buf.as_ref());
                        tx.unbounded_send(Ok(read_buf.freeze()))?;
                    }
                    Err(e) => {
                        // device error
                        log::error!("error in hid receive loop: {:?}", e);
                        tx.unbounded_send(Err(e))?;
                    }
                }
            }
        });
    }
}

impl Stream for HidStream {
    type Item = Result<Bytes, HidError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().rx.poll_next(cx)
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
    use futures::{SinkExt, StreamExt};

    use super::*;
    use crate::transport::hid::initialize_api;

    #[tokio::test]
    #[ignore]
    async fn test_hid() {
        let api = initialize_api().unwrap();
        let api = api.lock().unwrap();
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
        let api = api.lock().unwrap();
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
