//! TCP server compatible with the official mobile and desktop application
use core::panic;
use std::{
    net::{Ipv4Addr, ToSocketAddrs},
    str::FromStr,
    sync::Arc,
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

use crate::{device_manager::Device, App};

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

pub fn start_advertise(app: &App, cfg: Arc<config::TcpServer>) -> Result<(), anyhow::Error> {
    for srv in &app.config.tcp_servers {
        if let Some(ref advertise) = srv.advertise {
            let ip_address = Ipv4Addr::from_str(&advertise.ip)?;
            let hostname: Arc<str> = Arc::from(advertise.name.clone());
            let cfg = cfg.clone();
            let packet_fn = move || -> Option<discovery::DiscoveryPacket> {
                // Find a suitable device to forward this client to
                let device = device_matches(&cfg).ok()?;
                let device_info = device.device_info()?;

                let mut packet = discovery::DiscoveryPacket {
                    // The mac address is used to distinguish between devices on the *device list* page in the MiniDSP Device Console app
                    mac_address: [0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00],
                    ip_address,
                    hwid: device_info.hw_id,
                    dsp_id: device_info.dsp_version,
                    fw_major: device_info.fw_major,
                    fw_minor: device_info.fw_minor,
                    sn: ((device_info.serial - 900000) & 0xFFFF) as u16,
                    hostname: hostname.to_string(),
                };

                // Use a unique MAC by stitching the IP's bytes in the last 4 bytes
                packet.mac_address[2..].copy_from_slice(&packet.ip_address.octets());

                Some(packet)
            };

            let bind_addr = match &advertise.bind_address {
                None => None,
                Some(addr) => Some(addr.to_socket_addrs()?.next().ok_or_else(|| {
                    anyhow::anyhow!("bind adddress didn't resolve to a usable address")
                })?),
            };

            let interval = Duration::from_secs(1);
            tokio::spawn(discovery::server::advertise_packet(
                bind_addr, packet_fn, interval,
            ));
        }
    }
    Ok(())
}

pub async fn main(cfg: config::TcpServer) -> Result<(), MiniDSPError> {
    let app = super::APP.get().unwrap();
    let app = app.read().await;
    let cfg = Arc::new(cfg);

    let bind_address = cfg.bind_address.as_deref().unwrap_or("0.0.0.0:5333");

    if let Err(adv_err) = start_advertise(&app, cfg.clone()) {
        log::error!("error launching advertisement task: {:?}", adv_err);
    }

    let listener = TcpListener::bind(&bind_address).await?;
    log::info!("Listening on {}", &bind_address);
    loop {
        select! {
           result = listener.accept() => {
                let (stream, addr) = result?;
                log::info!("[{:?}] New connection", addr);

                // Find a suitable device to forward this client to
                let device = {
                    let cfg = cfg.clone();
                    tokio::task::spawn_blocking(move || device_matches(&cfg).ok()).await.unwrap()
                };

                log::info!("[{:?}] New connection assiged to {}",
                    addr,
                    device.as_ref().map(|dev| dev.url.clone()).unwrap_or_else(|| "(no devices found)".to_string())
                );

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

fn device_matches(cfg: &config::TcpServer) -> Result<Arc<Device>> {
    let app = super::APP.get().unwrap();
    let app = app.blocking_read();
    let device = {
        let mut devices = app
            .device_manager
            .as_ref()
            .ok_or(MiniDSPError::TransportClosed)?
            .devices();

        if let Some(serial) = cfg.device_serial {
            devices.into_iter().find(|dev| {
                dev.device_info()
                    .map(|di| di.serial == serial)
                    .unwrap_or(false)
            })
        } else if let Some(device_index) = cfg.device_index {
            devices.into_iter().nth(device_index)
        } else {
            devices.sort_by_key(|dev| !dev.is_local());
            devices.into_iter().next()
        }
    };

    device.ok_or_else(|| anyhow::anyhow!("no matching devices found"))
}
