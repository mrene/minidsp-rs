use super::async_wrapper::HidDeviceWrapper;
use crate::transport::{MiniDSPError, Sender, Transport};
use anyhow::Result;
use async_trait::async_trait;
use bytes::{BufMut, Bytes, BytesMut};
use hidapi::HidDevice;
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot, Mutex};

// 65 byte wide: 1 byte report id + 64 bytes data
const HID_PACKET_SIZE: usize = 65;

pub struct HidTransport {
    /// The receive loop will exit once this is dropped
    #[allow(dead_code)]
    shutdown_tx: oneshot::Sender<()>,

    /// The sending side of a broadcast channel used for received HID messages
    receiver_tx: broadcast::Sender<Bytes>,

    /// Inner struct wrapping the device handle, ensuring only one sender exists simultaneously
    /// The Arc is used to be able to hold a lock guard as 'static
    inner: Arc<Mutex<Inner>>,
}

impl HidTransport {
    pub fn new(device: HidDevice) -> Self {
        let device = Arc::new(HidDeviceWrapper::new(device));
        let (recv_send, _) = broadcast::channel::<Bytes>(10);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        // Spawn blocking send & recv loops.
        // TODO: Handle failures from recv loops
        tokio::spawn(HidTransport::recv_loop(
            recv_send.clone(),
            shutdown_rx,
            device.clone(),
        ));

        HidTransport {
            shutdown_tx,
            receiver_tx: recv_send,
            inner: Arc::new(Mutex::new(Inner::new(device))),
        }
    }

    async fn recv_loop(
        sender: broadcast::Sender<Bytes>,
        mut shutdown_rx: oneshot::Receiver<()>,
        device: Arc<HidDeviceWrapper>,
    ) -> Result<()> {
        use oneshot::error::TryRecvError::Empty;

        loop {
            // If the shutdown channel has been closed, bail out of the loop.
            match shutdown_rx.try_recv() {
                Err(Empty) => {}
                _ => return Ok(()),
            }

            let mut read_buf = BytesMut::with_capacity(HID_PACKET_SIZE);
            read_buf.resize(HID_PACKET_SIZE, 0);

            let size = tokio::task::block_in_place(|| device.read_timeout(&mut read_buf, 500));
            match size {
                // read_timeout returns Ok(0) if a timeout has occurred
                Ok(0) => {}

                // successful read
                Ok(size) => {
                    read_buf.truncate(size);

                    // println!("{:02x?}", read_buf.as_ref());

                    // Discard send errors, since it means there are no bound receiver
                    let _ = sender.send(read_buf.freeze());
                }

                // device error
                Err(e) => {
                    eprintln!("error in hid receive loop: {:?}", e);
                    return Err(e.into());
                }
            }
        }
    }
}

#[async_trait]
impl Transport for HidTransport {
    // type Sender = Inner;

    fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.receiver_tx.subscribe()
    }

    async fn send_lock(&'_ self) -> Box<dyn Sender> {
        return Box::new(self.inner.clone().lock_owned().await);
    }
}

pub struct Inner {
    device: Arc<HidDeviceWrapper>,

    /// Re-usable buffer for assembling the frame being written
    buf: BytesMut,
}

impl Inner {
    fn new(device: Arc<HidDeviceWrapper>) -> Self {
        Inner {
            device,
            buf: BytesMut::with_capacity(65),
        }
    }
}

#[async_trait]
impl Sender for Inner {
    async fn send(&mut self, frame: Bytes) -> Result<(), MiniDSPError> {
        self.buf.truncate(0);

        // HID report id
        self.buf.put_u8(0);

        // Frame data
        self.buf.extend_from_slice(&frame);

        // Pad remaining packet data with 0xFF
        self.buf.resize(HID_PACKET_SIZE, 0xFF);

        self.device
            .write(self.buf.as_ref())
            .map_err(MiniDSPError::HIDError)?;
        Ok(())
    }
}
