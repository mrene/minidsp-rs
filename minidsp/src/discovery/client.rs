use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Result;
use tokio::net::UdpSocket;
use tokio_util::udp::UdpFramed;

use super::DiscoveryPacketCodec;

/// Returns a stream with incoming discovery packets
pub async fn discover() -> Result<UdpFramed<DiscoveryPacketCodec>> {
    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 3999)).await?;
    let framed = UdpFramed::new(socket, DiscoveryPacketCodec {});
    Ok(framed)
}
