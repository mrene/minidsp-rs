use super::{DiscoveryEvent, Registry};
use anyhow::Result;
use atomic_refcell::AtomicRefCell;
use futures::StreamExt;
use gotham::handler;
use lazy_static::lazy_static;
use minidsp::{
    client::Client,
    device,
    transport::{self, SharedService},
    utils::{DropJoinHandle, ErrInto},
    DeviceInfo, MiniDSP,
};
use std::sync::{Arc, Weak};
use tokio::sync::{Mutex, RwLock};
use url2::Url2;

pub struct DeviceManager {
    inner: Arc<std::sync::RwLock<DeviceManagerInner>>,
    handles: Vec<DropJoinHandle<Result<(), anyhow::Error>>>,
}

impl DeviceManager {
    pub fn new(registry: Registry) -> Self {
        let inner = DeviceManagerInner {
            registry,
            ..Default::default()
        };

        let inner = Arc::new(std::sync::RwLock::new(inner));
        let mut handles = Vec::new();

        {
            // Start tasks for discovery processes
            let discovery_hid = {
                let inner = inner.clone();
                tokio::spawn(async move {
                    super::discovery::tasks::hid_discovery_task(|dev| {
                        let inner = inner.read().unwrap();
                        inner.registry.register(dev);
                    })
                    .await
                    .err_into()
                })
                .into()
            };
            handles.push(discovery_hid);

            let discovery_net = {
                let inner = inner.clone();
                tokio::spawn(async move {
                    super::discovery::tasks::net_discovery_task(|dev| {
                        let inner = inner.read().unwrap();
                        inner.registry.register(dev);
                    })
                    .await
                })
                .into()
            };
            handles.push(discovery_net);

            let task = {
                let inner = inner.clone();
                tokio::spawn(async move {
                    DeviceManager::task(inner).await;
                    Ok(())
                })
                .into()
            };
            handles.push(task);
        }

        DeviceManager { inner, handles }
    }

    async fn task(inner: Arc<std::sync::RwLock<DeviceManagerInner>>) {
        let mut discovery_events = {
            let inner = inner.read().unwrap();
            inner.registry.subscribe()
        };

        loop {
            while let Some(event) = discovery_events.next().await {
                log::trace!("{:?}", &event);

                let mut inner = inner.write().unwrap();
                match event {
                    DiscoveryEvent::Added(id) => {
                        inner.devices.push(Device::new(id));
                    }
                    DiscoveryEvent::Timeout { id, last_seen } => {
                        log::info!(
                            "Device hasn't been seen since timeout period: {} (last seen at {:?})",
                            id,
                            last_seen
                        );

                        // Remove that device from the list
                        inner.devices.retain(|d| !d.url.eq(id.as_str()));
                    }
                }
            }
        }
    }
}

#[derive(Default)]
pub struct DeviceManagerInner {
    registry: Registry,
    devices: Vec<Device>,
}

pub struct Device {
    url: String,
    inner: Arc<std::sync::RwLock<DeviceInner>>,
    handles: Vec<DropJoinHandle<Result<(), anyhow::Error>>>,
}

impl Device {
    pub fn new(url: String) -> Self {
        let inner = Arc::new(std::sync::RwLock::new(DeviceInner {
            url: url.clone(),
            ..Default::default()
        }));

        let mut handles = Vec::new();
        {
            let inner = inner.clone();
            let handle = tokio::spawn(async move {
                Device::task(inner).await
            });
            handles.push(handle.into());
        }

        Device {
            url,
            inner,
            handles,
        }
    }

    async fn task(inner: Arc<std::sync::RwLock<DeviceInner>>) -> anyhow::Result<()> {
        let url = {
            let inner = inner.read().unwrap();
            inner.url.clone()
        };

        log::info!("Connecting to {}", url.as_str());

        let transport = {
            let url = Url2::try_parse(url.as_str()).expect("Device::run had invalid url");
            let stream = transport::open_url(url).await?;
            transport::Hub::new(stream)
        };

        let service = {
            let mplex = transport::Multiplexer::from_transport(transport.clone());
            Arc::new(Mutex::new(mplex.to_service()))
        };

        let client = Client::new(service.clone());
        let device_info = client.get_device_info().await.ok();
        let device_spec = device_info
            .map(|dev| device::probe(&dev))
            .unwrap_or_default();

    
        let handle = DeviceHandle {
            service,
            transport,
            device_spec,
            device_info,
        };

        {
            let mut inner = inner.write().unwrap();
            inner.handle.replace(handle);
        }
        
        // TODO: Select things to make sure the device is still alive, exit once it's gone.

        Ok(())
    }
}
#[derive(Default, Clone)]
pub struct DeviceInner {
    url: String,
    handle: Option<DeviceHandle>
}

#[derive(Clone)]
pub struct DeviceHandle {
    // A pre-configured multiplexer ready to be bound to a `Client`
    pub service: SharedService,

    // Frame-level multiplexer
    pub transport: transport::Hub,

    // Probed hardware id and dsp version
    pub device_info: Option<DeviceInfo>,

    // Device spec structure indicating the address of every component
    pub device_spec: Option<&'static minidsp::device::Device>,
}

impl DeviceHandle {
    pub fn to_minidsp(&self) -> MiniDSP<'static> {
        // TODO: This should fail properly
        MiniDSP::new(
            self.service.clone(),
            self.device_spec.expect("device spec not available"),
        )
    }
}
