use anyhow::Result;
use atomic_refcell::AtomicRefCell;
use discovery::{DiscoveryEvent, Registry};
use futures::StreamExt;
use lazy_static::lazy_static;
use minidsp::{
    transport::{self, SharedService},
    utils::DropJoinHandle,
    MiniDSP,
};
use std::sync::{Arc, Weak};
use tokio::sync::{Mutex, RwLock};
use url2::Url2;

mod discovery;
mod http;

lazy_static! {
    /// The global application instance.
    /// Note: The rwlock is always to be read-locked except during initialization
    static ref APP: Arc<RwLock<App>> = App::new();
}

#[derive(Default)]
pub struct App {
    registry: Registry,
    handles: Option<Handles>,
    devices: RwLock<Vec<Arc<Device>>>,
}

impl App {
    pub fn new() -> Arc<RwLock<Self>> {
        let app = Arc::new(RwLock::new(Self {
            ..Default::default()
        }));

        {
            let mut app_mut = app.try_write().unwrap();
            // Start tasks for discovery processes
            let discovery_hid = {
                let app = app.clone();
                tokio::spawn(async move {
                    let app = app.read().await;
                    discovery::hid_discovery_task(&app.registry).await
                })
                .into()
            };

            let discovery_net = {
                let app = app.clone();
                tokio::spawn(async move {
                    let app = app.read().await;
                    discovery::net_discovery_task(&app.registry).await
                })
                .into()
            };
            app_mut.handles = Some(Handles {
                discovery_hid,
                discovery_net,
            });
        }

        app
    }
}

struct Handles {
    #[allow(dead_code)]
    discovery_hid: DropJoinHandle<Result<()>>,
    #[allow(dead_code)]
    discovery_net: DropJoinHandle<Result<()>>,
}

struct Device {
    // future: Option<Box<dyn Future<Output = Result<()>> + Send + 'static>>,
    /// Handle to the task controlling this device's lifecycle
    // handle: ,
    url: String,

    inner: RwLock<Option<Inner>>,

    #[allow(dead_code)]
    join_handle: AtomicRefCell<Option<DropJoinHandle<Result<()>>>>,
}

impl Device {
    pub fn start(url: String) -> Arc<Self> {
        let dev = Arc::new(Self {
            url,
            inner: RwLock::new(None),
            join_handle: AtomicRefCell::new(None),
        });

        let handle_dev = dev.clone();
        let handle = tokio::spawn(async move {
            let dev = Arc::downgrade(&handle_dev.clone());
            Device::run(dev).await
        });

        dev.join_handle
            .borrow_mut()
            .replace(DropJoinHandle::new(handle));
        dev
    }

    pub async fn run(this: Weak<Self>) -> anyhow::Result<()> {
        // This future is being dropped when the object is dropped
        // a weak reference is used in order to prevent a cycle, but we
        // can safely .unwrap() the weak ref since it the future wouldn't be running
        // if the object had been free'd

        // Open the transport by URL
        let url = {
            let this = this.upgrade().unwrap();
            this.url.clone()
        };

        log::info!("Connecting to {}", url.as_str());

        let transport = {
            let url = Url2::try_parse(url.as_str()).unwrap();
            let stream = transport::open_url(url).await?;
            transport::Hub::new(stream)
        };

        let service = {
            let mplex = transport::Multiplexer::from_transport(transport.clone());
            Arc::new(Mutex::new(mplex.to_service()))
        };

        // TODO: Detect device type
        let dev = MiniDSP::new(service.clone(), &minidsp::device::DEVICE_2X4HD);
        let info = dev.get_device_info().await?;
        println!("INFO: {:?}", &info);

        {
            let this = this.upgrade().unwrap();
            this.inner.write().await.replace(Inner {
                service,
                transport,
            });
        }

        Ok(())
    }
}

pub struct Inner {
    service: SharedService,
    transport: transport::Hub,
}

pub async fn discovery_task() {
    let app = APP.clone();
    let app = app.read().await;

    let mut discovery_events = app.registry.subscribe();

    loop {
        while let Some(event) = discovery_events.next().await {
            println!("{:?}", &event);
            match event {
                DiscoveryEvent::Added(id) => {
                    let mut devices = app.devices.write().await;
                    devices.push(Device::start(id));
                }
                DiscoveryEvent::Timeout { id, last_seen } => {
                    log::info!(
                        "Device hasn't been seen since timeout period: {} (last seen at {:?})",
                        id,
                        last_seen
                    );

                    // Remove that device from the list
                    let mut devices = app.devices.write().await;
                    devices.retain(|d| !d.url.eq(id.as_str()));
                }
            }
        }
    }
}

#[tokio::main]
pub async fn main() {
    env_logger::init();

    // Handle devices being discovered locally and on the network
    let mut _discovery_handle = DropJoinHandle::new(tokio::spawn(discovery_task()));
    let mut _http_handle = DropJoinHandle::new(tokio::spawn(http::main()));

    let _ = _discovery_handle.await;
}
