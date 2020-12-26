// #[macro_use]
extern crate log;
/// Main entrypoint for a server component
extern crate tokio;

use anyhow::{anyhow, Result};
use bytes::BytesMut;
use hidapi::{HidApi, HidDevice};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use minidsp::transport::hid::handle::HidTransport;
use minidsp::transport::Transport;

async fn forward(device: HidDevice, mut tcp: TcpStream) -> Result<()> {
    let handle = HidTransport::new(device);
    let mut device_receiver = handle.subscribe();

    loop {
        let mut tcp_recv_buf = BytesMut::with_capacity(65);
        tokio::select! {
            read_result = device_receiver.recv() => {
                match read_result {
                    Err(_) => { return Ok(()) },
                    Ok(read_buf) => {
                        let read_size = read_buf[0] as usize;
                        println!("hid: {:?} {:02x?}", read_size, &read_buf[..read_size]);
                        tcp.write_all(&read_buf[..read_size]).await?;
                    }
                }
            },
            recv_result = tcp.read_buf(&mut tcp_recv_buf) => {
                let recv_size = recv_result?;
                if recv_size == 0 {
                    return Ok(())
                }

                println!("tcp: {:?} {:02x?}", recv_size, &tcp_recv_buf);

                handle.send(tcp_recv_buf.freeze())
                    .await
                    .map_err(|_| anyhow!("send error"))?;
            },
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let listener = TcpListener::bind("0.0.0.0:5333").await?;

    loop {
        let (stream, addr) = listener.accept().await?;

        eprintln!("New connection: {:?}", addr);
        tokio::spawn(async move {
            let result: Result<()> = async {
                let (vid, pid) = (0x2752, 0x0011);
                let hid = HidApi::new()?;
                let hid_device = hid.open(vid, pid)?;
                forward(hid_device, stream).await
            }
            .await;

            if let Err(e) = result {
                eprintln!("err: {:?}", e)
            }

            eprintln!("Closed: {:?}", addr);
        });
    }
}
