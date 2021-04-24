use std::{collections::HashMap, sync::RwLock, time};

///! Device registry
/// This module is responsible for keeping track of discovered urls. Other components are free
/// to call `register` with urls that they want to probe, they will be added to the registry and events will fire,
/// triggering any external probing logic.
use futures::channel::mpsc;
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
    pub fn register(&self, dev: &str) {
        let mut inner = self.inner.write().unwrap();
        inner.register(dev)
    }

    pub fn remove(&self, dev: &str) {
        let mut inner = self.inner.write().unwrap();
        inner.remove(dev)
    }

    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<DiscoveryEvent> {
        let mut inner = self.inner.write().unwrap();
        inner.subscribe()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Registry::new()
    }
}
#[derive(Debug)]
pub struct Inner {
    pub devices: HashMap<String, Device>,
    pub sender: mpsc::UnboundedSender<DiscoveryEvent>,
    pub subscriber: Option<mpsc::UnboundedReceiver<DiscoveryEvent>>,
}

impl Inner {
    pub fn new() -> Self {
        let (sender, subscriber) = mpsc::unbounded();
        Self {
            devices: HashMap::new(),
            sender,
            subscriber: Some(subscriber),
        }
    }

    pub fn subscribe(&mut self) -> mpsc::UnboundedReceiver<DiscoveryEvent> {
        if self.subscriber.is_none() {
            panic!("a subscriber is already present");
        }

        self.subscriber.take().unwrap()
    }

    /// Adds a device to the list of reachable devices if it doesn't exist.
    pub fn register(&mut self, dev: &str) {
        let id = dev.to_string();
        let device = self.devices.get_mut(id.as_str());

        match device {
            None => {
                let _ = self
                    .sender
                    .unbounded_send(DiscoveryEvent::Added(id.clone()));
                self.devices.insert(id, Device::new());
            }
            Some(device) => device.mark_seen(),
        }

        self.cleanup();
    }

    pub fn remove(&mut self, dev: &str) {
        self.devices.retain(|k, _| k != dev);
    }

    /// Removes devices that haven't been seen since 5 minutes
    fn cleanup(&mut self) {
        let hid_devices = &mut self.devices;
        let sender = &self.sender;

        hid_devices.retain(|id, dev| {
            let keep = time::Instant::now().duration_since(dev.last_seen).as_secs() < 5 * 60;
            if !keep {
                let _ = sender.unbounded_send(DiscoveryEvent::Timeout {
                    id: id.to_string(),
                    last_seen: dev.last_seen,
                });
            }
            keep
        });
    }
}

#[derive(Debug)]
pub struct Device {
    pub last_seen: time::Instant,
}

impl Device {
    pub fn new() -> Self {
        Self {
            last_seen: time::Instant::now(),
        }
    }

    pub fn mark_seen(&mut self) {
        self.last_seen = time::Instant::now();
    }
}

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// A new device has been discovered and added to the list
    Added(String),

    /// A previously known device has not been seen since the timeout period
    Timeout {
        id: String,
        last_seen: time::Instant,
    },
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    pub async fn discovery() {
        let discovery = Registry::new();
        discovery.register("mock:");
        let inner = discovery.inner.read().unwrap();
        assert!(inner.devices.len() == 1);
    }
}
