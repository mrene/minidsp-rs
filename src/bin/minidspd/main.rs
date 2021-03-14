///! Main entrypoint
/// Launches the application by instantiating all components
///
use anyhow::Result;
use discovery::{DiscoveryEvent, Registry};
use lazy_static::lazy_static;
use minidsp::utils::OwnedJoinHandle;
use std::sync::Arc;
use tokio::sync::RwLock;

mod device_manager;
mod discovery;
mod http;
mod tcp;

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
        let mut handles = vec![];

        handles.push(
            tokio::spawn(async move {
                http::main().await;
                Ok(())
            })
            .into(),
        );

        handles.push(
            tokio::spawn(async move {
                tcp::main().await;
                Ok(())
            })
            .into(),
        );

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

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
