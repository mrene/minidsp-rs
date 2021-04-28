///! Device Manager: Reacts to discovery events, probe devices and make them ready for use by other components
use std::{
    net::IpAddr,
    sync::{Arc, RwLock, Weak},
};

use anyhow::{anyhow, Result};
use futures::{StreamExt, TryFutureExt};
use minidsp::{
    client::Client,
    device, logging,
    transport::{self, SharedService},
    utils::OwnedJoinHandle,
    DeviceInfo, MiniDSP,
};
use tokio::sync::Mutex;
use url2::Url2;

use super::discovery::{DiscoveryEvent, Registry};

pub struct DeviceManager {
    #[allow(dead_code)]
    inner: Arc<RwLock<DeviceManagerInner>>,
    #[allow(dead_code)]
    handles: Vec<OwnedJoinHandle<Result<(), anyhow::Error>>>,
}

impl DeviceManager {
    pub fn new(registry: Registry, ignore_net_ip: Option<IpAddr>) -> Self {
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
                tokio::spawn(
                    super::discovery::tasks::hid_discovery_task(move |dev| {
                        let inner = inner.read().unwrap();
                        inner.registry.register(dev);
                    })
                    .err_into(),
                )
                .into()
            };
            handles.push(discovery_hid);

            let discovery_net = {
                let inner = inner.clone();
                tokio::spawn(super::discovery::tasks::net_discovery_task(
                    move |dev| {
                        let inner = inner.read().unwrap();
                        inner.registry.register(dev);
                    },
                    ignore_net_ip,
                ))
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

    pub fn devices(&self) -> Vec<Arc<Device>> {
        let inner = self.inner.read().unwrap();
        inner.devices.clone()
    }

    async fn task(inner: Arc<RwLock<DeviceManagerInner>>) {
        let mut discovery_events = {
            let inner = inner.read().unwrap();
            inner.registry.subscribe()
        };

        loop {
            while let Some(event) = discovery_events.next().await {
                log::trace!("{:?}", &event);

                let weak_inner = Arc::downgrade(&inner);
                let mut inner = inner.write().unwrap();
                match event {
                    DiscoveryEvent::Added(id) => {
                        inner.devices.push(Device::new(id, weak_inner).into());
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
    devices: Vec<Arc<Device>>,
}

impl DeviceManagerInner {
    pub fn remove(&mut self, url: &str) {
        self.devices.retain(|dev| dev.url != url);
        self.registry.remove(url);
    }
}

pub struct Device {
    pub url: String,
    #[allow(dead_code)]
    inner: Arc<RwLock<DeviceInner>>,
    #[allow(dead_code)]
    handles: Vec<OwnedJoinHandle<Result<(), anyhow::Error>>>,
}

impl Device {
    pub fn new(url: String, device_manager: Weak<RwLock<DeviceManagerInner>>) -> Self {
        let inner = Arc::new(std::sync::RwLock::new(DeviceInner {
            url: url.clone(),
            device_manager,
            ..Default::default()
        }));

        let mut handles = Vec::new();
        {
            let inner = inner.clone();
            let handle = tokio::spawn(async move { Device::task(inner).await });
            handles.push(handle.into());
        }

        Device {
            url,
            inner,
            handles,
        }
    }

    pub fn is_local(&self) -> bool {
        self.url.starts_with("usb:")
    }

    pub fn to_hub(&self) -> Option<transport::Hub> {
        let inner = self.inner.read().unwrap();
        inner.handle.as_ref()?.to_hub()
    }

    pub fn to_minidsp(&self) -> Option<MiniDSP<'static>> {
        let inner = self.inner.read().unwrap();
        inner.handle.as_ref()?.to_minidsp()
    }

    pub fn device_info(&self) -> Option<DeviceInfo> {
        let inner = self.inner.read().unwrap();
        inner.handle.as_ref()?.device_info
    }

    pub fn device_spec(&self) -> Option<&'static minidsp::device::Device> {
        let inner = self.inner.read().unwrap();
        inner.handle.as_ref()?.device_spec
    }

    async fn task_inner(inner: Arc<RwLock<DeviceInner>>) -> anyhow::Result<()> {
        let url = {
            let inner = inner.read().unwrap();
            inner.url.clone()
        };

        log::info!("Connecting to {}", url.as_str());

        // Connect to the device by url, and get a frame-level transport
        let mut transport = {
            let url = Url2::try_parse(url.as_str()).expect("Device::run had invalid url");
            let stream = transport::open_url(&url).await?;

            // If we have any logging options, log this stream
            let app = super::APP.get().unwrap();
            let app = app.read().await;
            let (_, stream) =
                logging::transport_logging(stream, app.opts.verbose as u8, app.opts.log.clone());

            transport::Hub::new(stream)
        };

        // Wrap the transport into a multiplexed service for command-level multiplexing
        let service = {
            let transport = transport
                .try_clone()
                .ok_or_else(|| anyhow!("transport closed prematurely"))?;
            let mplex = transport::Multiplexer::from_transport(transport);
            Arc::new(Mutex::new(mplex.to_service()))
        };

        // Probe the device hardware id and dsp version in order to get the right specs
        // Keep going if we do not know the device type, but it has successfully responsed to
        // probing commands. This can be used to support a common subset of features without
        // knowing the device-specific memory layout.
        let client = Client::new(service.clone());
        let device_info = client.get_device_info().await.ok();
        let device_spec = device_info.map(|dev| device::probe(&dev));

        let handle = DeviceHandle {
            service,
            transport: transport
                .try_clone()
                .ok_or_else(|| anyhow!("transport closed prematurely"))?,
            device_spec,
            device_info,
        };

        log::info!(
            "Identified {} as {} (serial# {})",
            &url,
            device_spec
                .map(|spec| spec.product_name)
                .unwrap_or("(unknown device)"),
            device_info
                .map(|di| format!("{}", di.serial))
                .unwrap_or_else(|| "unknown".to_string())
        );

        {
            let mut inner = inner.write().unwrap();
            inner.handle.replace(handle);
        }

        // Keep reading messages until the device returns an error/eof
        while let Some(frame) = transport.next().await {
            if let Err(e) = frame {
                log::warn!("Device at {} closing to to an error: {}", &url, &e);
                break;
            }
        }

        log::warn!("Device at {} is closing (EOF)", &url);

        // Notify the device manager that this device is to be removed
        if let Some(device_manager) = inner.read().unwrap().device_manager.upgrade() {
            let mut device_manager = device_manager.write().unwrap();
            device_manager.remove(&url);
        }

        Ok(())
    }

    /// Main device task
    /// This is spawned when the device is first discovered and manages it's complete lifecycle.
    async fn task(inner: Arc<RwLock<DeviceInner>>) -> anyhow::Result<()> {
        loop {
            // Try to probe the device until we're successful
            if Self::task_inner(inner.clone()).await.is_ok() {
                return Ok(());
            }

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}
#[derive(Default)]
pub struct DeviceInner {
    url: String,
    handle: Option<DeviceHandle>,

    device_manager: Weak<RwLock<DeviceManagerInner>>,
}

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
    pub fn to_minidsp(&self) -> Option<MiniDSP<'static>> {
        let mut dsp = MiniDSP::new(self.service.clone(), self.device_spec?);
        if let Some(device_info) = self.device_info {
            dsp.set_device_info(device_info);
        }
        Some(dsp)
    }

    pub fn to_hub(&self) -> Option<transport::Hub> {
        self.transport.try_clone()
    }
}
