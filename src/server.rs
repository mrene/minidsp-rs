//! TCP server compatible with the official mobile and desktop application
// #[macro_use]
extern crate log;
use crate::transport::Transport;
use anyhow::{anyhow, Result};
use bytes::BytesMut;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Forwards the given tcp stream to a transport.
/// This lets multiple users talk to the same device simultaneously, which depending on the
/// user could be problematic.
async fn forward(handle: Arc<dyn Transport>, mut tcp: TcpStream) -> Result<()> {
    let mut device_receiver = handle.subscribe();

    loop {
        let mut tcp_recv_buf = BytesMut::with_capacity(65);
        tokio::select! {
            read_result = device_receiver.recv() => {
                match read_result {
                    Err(_) => { return Ok(()) },
                    Ok(read_buf) => {
                        let read_size = read_buf[0] as usize;
                        tcp.write_all(&read_buf[..read_size]).await?;
                    }
                }
            },
            recv_result = tcp.read_buf(&mut tcp_recv_buf) => {
                let recv_size = recv_result?;
                if recv_size == 0 {
                    return Ok(())
                }

                handle.send(tcp_recv_buf.freeze())
                    .await
                    .map_err(|_| anyhow!("send error"))?;
            },
        }
    }
}

/// Listen and forward every incoming tcp connection to the given transport
pub async fn serve(bind_address: String, transport: Arc<dyn Transport>) -> Result<()> {
    let listener = TcpListener::bind(bind_address).await?;

    loop {
        let (stream, addr) = listener.accept().await?;
        let handle = transport.clone();
        eprintln!("New connection: {:?}", addr);
        tokio::spawn(async move {
            let result: Result<()> = async { forward(handle, stream).await }.await;

            if let Err(e) = result {
                eprintln!("err: {:?}", e);
            }

            eprintln!("Closed: {:?}", addr);
        });
    }
}
