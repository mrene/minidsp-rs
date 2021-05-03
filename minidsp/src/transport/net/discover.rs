//! Allows talking with the [crate::server] component
use std::{collections::HashMap, fmt, net::SocketAddr, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use tokio::net::TcpStream;

use super::{discovery, StreamTransport};
use crate::transport::{IntoTransport, MiniDSPError, Openable, Transport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub packet: discovery::DiscoveryPacket,
    pub ip: SocketAddr,
}

impl Device {
    pub fn to_url(&self) -> String {
        ToString::to_string(&self)
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tcp://{}:5333?name={}",
            self.ip.ip(),
            urlencoding::encode(self.packet.hostname.as_str())
        )
    }
}

#[async_trait]
impl Openable for Device {
    async fn open(&self) -> Result<Transport, MiniDSPError> {
        Ok(
            StreamTransport::new(TcpStream::connect(SocketAddr::new(self.ip.ip(), 5333)).await?)
                .into_transport(),
        )
    }

    fn to_url(&self) -> String {
        self.to_string()
    }
}

pub async fn discover() -> Result<impl Stream<Item = Device>> {
    let stream = discovery::client::discover().await?;
    Ok(stream.filter_map(|item| async {
        let (packet, ip) = item.ok()?;
        Some(Device { packet, ip })
    }))
}

/// Gather discovery packets during the timeout period and return a de-duplicated list by ip
pub async fn discover_timeout(timeout: Duration) -> Result<Vec<Device>, anyhow::Error> {
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
