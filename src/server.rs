//! TCP server compatible with the official mobile and desktop application
// #[macro_use]
extern crate log;

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

use crate::utils::decoder::Decoder;
use crate::{transport::Transport, MiniDSPError};

/// Forwards the given tcp stream to a transport.
/// This lets multiple users talk to the same device simultaneously, which depending on the
/// user could be problematic.
async fn forward(handle: Arc<dyn Transport>, mut tcp: TcpStream) -> Result<()> {
    let mut device_receiver = handle.subscribe()?;

    let decoder = {
        use termcolor::{ColorChoice, StandardStream};
        let writer = StandardStream::stderr(ColorChoice::Always);
        Arc::new(Mutex::new(Decoder {
            w: Box::new(writer),
            quiet: true,
        }))
    };

    loop {
        let mut tcp_recv_buf = BytesMut::with_capacity(65);
        tokio::select! {
            read_result = device_receiver.recv() => {
                match read_result {
                    Err(_) => { return Ok(()) },
                    Ok(read_buf) => {
                        let read_size = read_buf[0] as usize;
                        {
                            decoder.lock().unwrap().feed_recv(&Bytes::copy_from_slice(&read_buf[..read_size]));
                        }
                        tcp.write_all(&read_buf[..read_size]).await?;
                    }
                }
            },
            recv_result = tcp.read_buf(&mut tcp_recv_buf) => {
                let recv_size = recv_result?;
                if recv_size == 0 {
                    return Ok(())
                }

                let tcp_recv_buf = tcp_recv_buf.freeze();
                {
                    decoder.lock().unwrap().feed_sent(&tcp_recv_buf);
                }
                handle.send(tcp_recv_buf)
                    .await
                    .map_err(|_| anyhow!("send error"))?;
            },
        }
    }
}

/// Listen and forward every incoming tcp connection to the given transport
pub async fn serve<A: ToSocketAddrs>(bind_address: A, transport: Arc<dyn Transport>) -> Result<()> {
    let listener = TcpListener::bind(bind_address).await?;
    let mut rx = transport.subscribe()?;

    loop {
        tokio::select! {
           result = listener.accept() => {
                let (stream, addr) = result?;
                let handle = transport.clone();
                eprintln!("New connection: {:?}", addr);
                tokio::spawn(async move {
                    let result: Result<()> = async { forward(handle, stream).await }.await;

                    if let Err(e) = result {
                        eprintln!("err: {:?}", e);
                    }

                    eprintln!("Closed: {:?}", addr);
                });
           },
           result = rx.recv() => {
                if result.is_err() {
                    return Err(MiniDSPError::TransportClosed.into())
                }
           }
        }
    }
}
