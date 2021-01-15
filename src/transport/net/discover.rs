//! Allows talking with the [crate::server] component
use crate::discovery;
use crate::transport::{MiniDSPError, Openable, Transport};
use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use std::{collections::HashMap, fmt, net::SocketAddr, time::Duration};
use tokio::net::TcpStream;

use super::StreamTransport;

pub struct Device {
    pub packet: discovery::DiscoveryPacket,
    pub ip: SocketAddr,
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:5333 {}", self.ip.ip(), self.packet.hostname)
    }
}

#[async_trait]
impl Openable for Device {
    async fn open(&self) -> Result<Transport, MiniDSPError> {
        Ok(StreamTransport::new(
            TcpStream::connect(SocketAddr::new(self.ip.ip(), 5333)).await?,
        ))
    }
}

/// Gather discovery packets during the timeout period and return a de-duplicated list by ip
pub async fn discover(timeout: Duration) -> Result<Vec<Device>, anyhow::Error> {
    let mut devices = Box::new(HashMap::new());
    let mut stream = discovery::client::discover().await?;

    let timeout = tokio::time::sleep(timeout);
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some(Ok((packet, ip))) = stream.next() => {
                devices.insert(ip, Device { packet, ip });
            },
            _ = &mut timeout => break,
        }
    }

    Ok(devices.drain().map(|(_, v)| v).collect())
}
