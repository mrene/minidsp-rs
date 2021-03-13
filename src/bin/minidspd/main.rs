use anyhow::Result;
use discovery::{DiscoveryEvent, Registry};
use lazy_static::lazy_static;
use minidsp::utils::OwnedJoinHandle;
use std::sync::Arc;
use tokio::sync::RwLock;

mod device_manager;
mod discovery;
mod http;

lazy_static! {
    /// The global application instance.
    static ref APP: Arc<RwLock<App>> = Arc::new(App::new().into());
}

pub struct App {
    #[allow(dead_code)]
    device_manager: device_manager::DeviceManager,
    #[allow(dead_code)]
    handles: Vec<OwnedJoinHandle<Result<(), anyhow::Error>>>,
}

impl App {
    pub fn new() -> Self {
        let registry = Registry::new();
        let device_mgr = device_manager::DeviceManager::new(registry);
        let handles = vec![];

        App {
            device_manager: device_mgr,
            handles,
        }
    }
}

impl Default for App {
    fn default() -> Self {
        App::new()
    }
}

#[tokio::main]
pub async fn main() {
    env_logger::init();
    let _ = APP.clone();
    // let _app = app.read().await;

    http::main().await;

    // Handle devices being discovered locally and on the network
    // app.handles.first().unwrap()
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
