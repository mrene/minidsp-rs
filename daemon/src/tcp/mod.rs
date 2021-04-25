//! TCP server compatible with the official mobile and desktop application
use std::{net::Ipv4Addr, str::FromStr, time::Duration};

use anyhow::{Context, Result};
use futures::{pin_mut, SinkExt, StreamExt};
use minidsp::{
    transport::{
        net::{discovery, Codec},
        Transport,
    },
    MiniDSPError,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    select,
};
use tokio_util::codec::Framed;

use super::{config, Opts};

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

pub fn start_advertise(opts: &Opts) -> Result<(), anyhow::Error> {
    if let Some(ref hostname) = opts.advertise {
        let mut packet = discovery::DiscoveryPacket {
            mac_address: [10, 20, 30, 40, 50, 60],
            ip_address: Ipv4Addr::UNSPECIFIED,
            hwid: 10,
            typ: 0,
            sn: 65535,
            hostname: hostname.to_string(),
        };
        if let Some(ref ip) = opts.ip {
            packet.ip_address = Ipv4Addr::from_str(ip.as_str())?;
        }
        let interval = Duration::from_secs(1);
        tokio::spawn(discovery::server::advertise_packet(packet, interval));
    }
    Ok(())
}

pub async fn main(cfg: config::TcpServer) -> Result<(), MiniDSPError> {
    let app = super::APP.get().unwrap();
    let app = app.read().await;

    if let Err(adv_err) = start_advertise(&app.opts) {
        log::error!("error launching advertisement task: {:?}", adv_err);
    }

    let bind_address = cfg
        .bind_address
        .unwrap_or_else(|| "0.0.0.0:5333".to_string());
    let listener = TcpListener::bind(&bind_address).await?;
    log::info!("Listening on {}", &bind_address);
    loop {
        select! {
           result = listener.accept() => {
                let (stream, addr) = result?;
                log::info!("[{:?}] New connection", addr);

                // TODO: There could be an unavailable local device before a legitimate one, this should
                // get the first device that returns a valid transport::Hub
                let device = {
                    let mut devices = app
                        .device_manager
                        .as_ref()
                        .ok_or(MiniDSPError::TransportClosed)?
                        .devices();
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
