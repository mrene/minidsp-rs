use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use anyhow::Result;
use log::{error, trace};
use tokio::{
    net::UdpSocket,
    time::{sleep, Duration},
};

use super::{DiscoveryPacket, DISCOVERY_PORT};

/// Advertises the given discovery packet at a specified interval
pub async fn advertise_packet(
    bind_addr: Option<SocketAddr>,
    packet: impl Fn() -> Option<DiscoveryPacket> + Send + Sync + 'static,
    interval: Duration,
) -> Result<()> {
    let bind_addr =
        bind_addr.unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));
    let socket = UdpSocket::bind(bind_addr).await?;
    socket.set_broadcast(true)?;

    let target_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), DISCOVERY_PORT);
    socket.connect(target_address).await?;

    let packet = Arc::new(packet);
    loop {
        let packet = packet.clone();
        let packet = tokio::task::spawn_blocking(move || packet()).await.unwrap();
        if let Some(packet) = packet {
            let packet_bytes = packet.to_bytes();
            let send_result = socket.send(packet_bytes.as_ref()).await;
            match send_result {
                Ok(_) => {
                    trace!("sent advertisement: {:?}", &packet);
                }
                Err(e) => {
                    error!("couldn't send broadcast datagram: {:?}", e);
                }
            }
        }
        sleep(interval).await;
    }
}
