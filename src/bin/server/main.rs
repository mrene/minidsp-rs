/// Main entrypoint for a server component
extern crate tokio;

// #[macro_use]
extern crate log;

use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use hidapi::{HidDevice, HidApi};
use tokio::sync::mpsc;
use std::sync::Arc;
use std::ops::Deref;

type HIDBuf = [u8; 65];

struct HidDeviceWrapper {
    pub inner: HidDevice,
}

impl HidDeviceWrapper {
    pub fn new(inner: HidDevice) -> Self {
        HidDeviceWrapper { inner }
    }
}

impl Deref for HidDeviceWrapper{
    type Target = HidDevice;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

unsafe impl Sync for HidDeviceWrapper {}

unsafe impl Send for HidDeviceWrapper {}

async fn hid_recv_loop(sender: mpsc::Sender<HIDBuf>, device: Arc<Box<HidDeviceWrapper>>) -> Result<(), failure::Error> {
    let mut hid_recv_send = sender;
    loop {
        let mut read_buf = [0u8; 65];

        tokio::task::block_in_place(
                || device.read(&mut read_buf))?;

        hid_recv_send.send(read_buf)
            .await
            .map_err(|_| failure::format_err!("send error"))?;

        // println!("hid: {:02x?}", &read_buf[..]);
    }
}

async fn hid_send_loop(receiver: mpsc::Receiver<HIDBuf>, device: Arc<Box<HidDeviceWrapper>>) {
    let mut hid_send_recv = receiver;

    loop {
        match hid_send_recv.recv().await {
            None => return,
            Some(send_buf) => {
                if let Err(e) = device.write(&send_buf) {
                    eprintln!("hid write error: {:?}", e);
                    return;
                }
            }
        }
    }
}

async fn forward(device: HidDevice, mut tcp: TcpStream) -> Result<(), failure::Error> {
    // let mut device = Box::new(hidapi_async::Device::new(device)?);/

    let device = Arc::new(Box::new(HidDeviceWrapper::new(device)));
    let (hid_recv_send, mut hid_recv_recv) = mpsc::channel::<HIDBuf>(10);
    {
        let device = device.clone();
        tokio::spawn(async move { hid_recv_loop(hid_recv_send, device).await });
    }

    let (mut hid_send_send, hid_send_recv) = mpsc::channel::<HIDBuf>(10);
    {
        let device = device.clone();
        tokio::spawn(async move { hid_send_loop(hid_send_recv, device).await });
    }

    loop {
        let mut tcp_recv_buf = [0u8; 65];
        tokio::select! {
            read_result = hid_recv_recv.recv() => {
                match read_result {
                    None => { return Ok(()) },
                    Some(read_buf) => {
                        let read_size = read_buf[0] as usize;
                        println!("hid: {:?} {:02x?}", read_size, &read_buf[..read_size]);
                        tcp.write_all(&read_buf[..read_size]).await?;
                    }
                }
            },
            recv_result = tcp.read(&mut tcp_recv_buf[1..]) => {
                let recv_size = recv_result?;
                if recv_size == 0 {
                    return Ok(())
                }

                tcp_recv_buf[0] = 0;
                println!("tcp: {:?} {:02x?}", recv_size, &tcp_recv_buf[..recv_size]);

                hid_send_send.send(tcp_recv_buf)
                    .await
                    .map_err(|_| failure::format_err!("send error"))?;
            }
        }
    }
}


#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    env_logger::init();

    let mut listener = TcpListener::bind("0.0.0.0:5333").await?;

    loop {
        let (stream, addr) = listener.accept().await?;

        eprintln!("New connection: {:?}", addr);
        tokio::spawn(async move {
            let result: Result<(), failure::Error> = async {
                let (vid, pid) = (0x2752, 0x0011);
                let hid = HidApi::new()?;
                let hid_device = hid.open(vid, pid)?;
                forward(hid_device, stream).await
            }.await;

            if let Err(e) = result {
                eprintln!("err: {:?}", e)
            }

            eprintln!("Closed: {:?}", addr);
        });
    }
}
