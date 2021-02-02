//! Device discovery
//! Maintains an up-to-date list of devices that can be reached,
//! clears up devices after they haven't been seen in 5 minutes

use anyhow::Result;
use futures::{pin_mut, StreamExt};
use log::warn;
use minidsp::transport::{self, Openable};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time,
};

pub struct Registry {
    pub inner: RwLock<Inner>,
}

impl Registry {
    pub fn new() -> Self {
        let inner = Inner::new();
        Self {
            inner: RwLock::new(inner),
        }
    }

    /// Adds a device to the list of reachable devices if it doesn't exist.
    pub fn register<Dev>(&self, dev: Dev)
    where
        Dev: Openable + Send + Sync + 'static,
    {
        let mut inner = self.inner.write().unwrap();
        let id = dev.to_string();
        let device = inner.hid_devices.get_mut(id.as_str());
        match device {
            None => {
                inner.hid_devices.insert(id, Device::new(Box::new(dev)));
            }
            Some(device) => device.mark_seen(),
        }

        inner.cleanup();
    }
}

impl Default for Registry {
    fn default() -> Self {
        Registry::new()
    }
}

pub struct Inner {
    // This should be a per-device state machine
    pub hid_devices: HashMap<String, Device>,
}

impl Inner {
    pub fn new() -> Self {
        Self {
            hid_devices: HashMap::new(),
        }
    }

    /// Removes devices that haven't been seen since 5 minutes
    fn cleanup(&mut self) {
        self.hid_devices
            .retain(|_, dev| time::Instant::now().duration_since(dev.last_seen).as_secs() < 5 * 60);
    }
}
pub struct Device {
    pub openable: Box<dyn transport::Openable + Send + Sync + 'static>,
    pub last_seen: time::Instant,
}

impl Device {
    pub fn new(openable: Box<dyn transport::Openable + Send + Sync + 'static>) -> Self {
        Self {
            openable,
            last_seen: time::Instant::now(),
        }
    }

    pub fn mark_seen(&mut self) {
        self.last_seen = time::Instant::now();
    }
}

pub async fn hid_discovery_task(registry: &Registry) -> Result<()> {
    let api = transport::hid::initialize_api()?;
    loop {
        match transport::hid::discover(&api) {
            Ok(devices) => {
                for device in devices {
                    registry.register(device);
                }
            }
            Err(e) => {
                warn!("failed to enumerate hid devices: {}", e);
            }
        }

        tokio::time::sleep(time::Duration::from_secs(5)).await;
    }
}

pub async fn net_discovery_task(registry: &Registry) -> Result<()> {
    let stream = transport::net::discover().await?;
    pin_mut!(stream);
    while let Some(device) = stream.next().await {
        registry.register(device);
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use async_trait::async_trait;
    use minidsp::transport::{MiniDSPError, Openable};

    use super::*;

    pub struct MockOpenable {
        id: String,
    }

    #[async_trait]
    impl Openable for MockOpenable {
        async fn open(
            &self,
        ) -> anyhow::Result<minidsp::transport::Transport, minidsp::transport::MiniDSPError>
        {
            Err(MiniDSPError::TransportClosed)
        }

        fn to_string(&self) -> String {
            self.id.clone()
        }
    }

    #[tokio::test]
    pub async fn discovery() {
        let discovery = Registry::new();

        let mock = MockOpenable {
            id: "id".to_string(),
        };
        discovery.register(mock);

        let inner = discovery.inner.read().unwrap();
        assert!(inner.hid_devices.len() == 1);
    }
}
