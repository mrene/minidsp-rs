//! TCP server compatible with the official mobile and desktop application
use std::{
    net::{Ipv4Addr, SocketAddr, ToSocketAddrs},
    str::FromStr,
    time::Duration,
};

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

use super::config;

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

pub fn start_advertise(
    bind_addr: SocketAddr,
    config: &config::Config,
) -> Result<(), anyhow::Error> {
    for srv in &config.tcp_servers {
        if let Some(ref advertise) = srv.advertise {
            let packet = discovery::DiscoveryPacket {
                mac_address: [10, 20, 30, 40, 50, 60],
                ip_address: Ipv4Addr::from_str(&advertise.ip)?,
                hwid: 27,
                fw_major: 1,
                fw_minor: 53,
                dsp_id: 0,
                sn: 65535,
                hostname: advertise.name.to_string(),
            };
            let interval = Duration::from_secs(1);
            tokio::spawn(discovery::server::advertise_packet(
                bind_addr, packet, interval,
            ));
        }
    }
    Ok(())
}

pub async fn main(cfg: config::TcpServer) -> Result<(), MiniDSPError> {
    let app = super::APP.get().unwrap();
    let app = app.read().await;

    let bind_address = cfg
        .bind_address
        .unwrap_or_else(|| "0.0.0.0:5333".to_string());

    let bind_addr = bind_address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow::anyhow!("bind adddress didn't resolve to a usable address"))?;

    if let Err(adv_err) = start_advertise(bind_addr, &app.config) {
        log::error!("error launching advertisement task: {:?}", adv_err);
    }

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

                    if let Some(serial) = cfg.device_serial {
                        devices.into_iter().find(|dev| dev.device_info().map(|di| di.serial == serial).unwrap_or(false))
                    } else if let Some(device_index) = cfg.device_index {
                        devices.into_iter().nth(device_index)
                    } else {
                        devices.sort_by_key(|dev| !dev.is_local());
                        devices.into_iter().next()
                    }
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
