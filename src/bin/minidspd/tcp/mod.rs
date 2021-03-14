//! TCP server compatible with the official mobile and desktop application
use anyhow::{Context, Result};
use futures::{pin_mut, SinkExt, StreamExt};
use minidsp::{
    transport::{net::Codec, Transport},
    MiniDSPError,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    select,
};
use tokio_util::codec::Framed;

/// This lets multiple users talk to the same device simultaneously, which depending on the
/// user could be problematic.
async fn forward<T>(tcp: T, device: Transport) -> Result<()>
where
    T: AsyncRead + AsyncWrite + 'static,
{
    // Truncate each HID frame after its ending
    let mut device = device.map(|frame| {
        let frame = frame?;
        if frame.is_empty() {
            return Err(MiniDSPError::MalformedResponse(
                "Received an empty frame".to_string(),
            ));
        }

        let len = frame[0] as usize;
        if frame.len() < len {
            return Err(MiniDSPError::MalformedResponse(format!(
                "Expected frame of length {}, got {}",
                len,
                frame.len()
            )));
        }

        Ok::<_, MiniDSPError>(frame.slice(0..len))
    });

    // Apply framing to the TCP stream
    let remote = Framed::new(tcp, Codec::new_server());
    pin_mut!(remote);

    loop {
        select! {
            frame = device.next() => {
                match frame {
                    Some(frame) => remote.send(frame?).await.context("remote.send failed")?,
                    None => {
                        return Err(MiniDSPError::TransportClosed.into());
                    }
                }
            },
            frame = remote.next() => {
                let frame = frame.ok_or(MiniDSPError::TransportClosed)?;
                device.send(frame.context("decoding frame")?).await.context("device_tx.send failed")?;
            },
        }
    }
}

pub async fn main() -> Result<(), MiniDSPError> {
    let app = super::APP.clone();
    let app = app.read().await;

    // TODO: Move udp broadcast advertisement here

    // TODO: Let the bind address be customized in the config
    let bind_address = "0.0.0.0:5333";
    let listener = TcpListener::bind(bind_address).await?;
    log::info!("Listening on {}", bind_address);
    loop {
        select! {
           result = listener.accept() => {
                let (stream, addr) = result?;
                log::info!("[{:?}] New connection", addr);

                // TODO: There could be an unavailable local device before a legitimate one, this should
                // get the first device that returns a valid transport::Hub
                let device = {
                    let mut devices = app.device_manager.devices();
                    devices.sort_by_key(|dev| !dev.is_local());
                    devices.first().cloned()
                };

                log::info!("[{:?}] New connection assiged to {}",
                    addr,
                    device.as_ref().map(|dev| dev.url.clone()).unwrap_or_else(|| "(no devices found)".to_string())
                );

                // Find a suitable device to forward this client to
                if let Some(hub) = device.and_then(|dev| dev.to_hub()) {
                    tokio::spawn(async move {
                        let result = forward(stream, Box::pin(hub)).await;

                        if let Err(e) = result {
                            log::info!("[{}] Connection closed: {:?}", addr, e);
                        }

                        log::info!("[{:?}] Closed", addr);
                    });
                }
           },
        }
    }
}
