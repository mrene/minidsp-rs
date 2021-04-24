use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Result;
use log::{error, trace};
use tokio::{
    net::UdpSocket,
    time::{sleep, Duration},
};

use super::{DiscoveryPacket, DISCOVERY_PORT};

/// Advertises the given discovery packet at a specified interval
pub async fn advertise_packet(packet: DiscoveryPacket, interval: Duration) -> Result<()> {
    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0u16)).await?;
    socket.set_broadcast(true)?;

    let target_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), DISCOVERY_PORT);
    socket.connect(target_address).await?;

    let packet_bytes = packet.to_bytes();
    loop {
        let send_result = socket.send(packet_bytes.as_ref()).await;
        match send_result {
            Ok(_) => {
                trace!("sent advertisement: {:?}", &packet);
            }
            Err(e) => {
                error!("couldn't send broadcast datagram: {:?}", e);
            }
        }
        sleep(interval).await;
    }
}
