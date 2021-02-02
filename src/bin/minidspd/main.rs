use anyhow::Result;
use discovery::Registry;
use lazy_static::lazy_static;
use minidsp::utils::DropJoinHandle;
use std::sync::Arc;
use tokio::sync::RwLock;

mod discovery;

lazy_static! {
    /// The global application instance.
    /// Note: The rwlock is always to be locked for read except during initialization
    static ref APP: Arc<RwLock<App>> = App::new();
}

#[derive(Default)]
pub struct App {
    registry: Registry,
    handles: Option<Handles>,
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

#[tokio::main]
pub async fn main() {
    let app = APP.clone();

    loop {
        // Print device list
        {
            let app = app.read().await;
            let registry = app.registry.inner.read().unwrap();
            let devices = registry.hid_devices.iter();
            for dev in devices {
                println!("{}", dev.0);
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
