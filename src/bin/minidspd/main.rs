///! Main entrypoint
/// Launches the application by instantiating all components
///
use anyhow::Result;
use clap::Clap;
use discovery::{DiscoveryEvent, Registry};
use lazy_static::lazy_static;
use minidsp::utils::OwnedJoinHandle;
use std::{path::PathBuf, str::FromStr, sync::Arc};
use tokio::sync::RwLock;

mod device_manager;
mod discovery;
mod http;
mod logging;
mod tcp;

lazy_static! {
    /// The global application instance.
    static ref APP: Arc<RwLock<App>> = Arc::new(App::new().into());
}

#[derive(Clone, Clap, Debug, Default)]
#[clap(version=env!("CARGO_PKG_VERSION"), author=env!("CARGO_PKG_AUTHORS"))]
pub struct Opts {
    /// Verbosity level. -v display decoded commands and responses -vv display decoded commands including readfloats -vvv display hex data frames
    #[clap(short, long, parse(from_occurrences))]
    verbose: i32,

    /// Log commands and responses to a file
    #[clap(long, env = "MINIDSP_LOG")]
    log: Option<PathBuf>,

    /// Bind address for the TCP server component
    #[clap(default_value = "0.0.0.0:5333")]
    bind_address: String,

    /// Bind address for the HTTP server
    #[clap(long)]
    http: Option<String>,

    /// If set, advertises the TCP component so it's discoverable from minidsp apps, using the given device name
    #[clap(long)]
    advertise: Option<String>,

    /// IP to use when advertising, required if --advertise is set
    #[clap(long)]
    ip: Option<String>,
}

pub struct App {
    opts: Opts,
    #[allow(dead_code)]
    device_manager: device_manager::DeviceManager,
    #[allow(dead_code)]
    handles: Vec<OwnedJoinHandle<Result<(), anyhow::Error>>>,
}

impl App {
    pub fn new() -> Self {
        let opts: Opts = Opts::parse();
        let registry = Registry::new();

        // If we're advertising a device, make sure to avoid discovering ourselves
        let this_ip = opts
            .ip
            .as_ref()
            .and_then(|ip| std::net::IpAddr::from_str(ip.as_str()).ok());

        let device_mgr = device_manager::DeviceManager::new(registry, this_ip);
        let mut handles = vec![];

        handles.push(
            tokio::spawn(async move {
                http::main().await?;
                Ok(())
            })
            .into(),
        );

        handles.push(
            tokio::spawn(async move {
                tcp::main().await?;
                Ok(())
            })
            .into(),
        );

        App {
            device_manager: device_mgr,
            handles,
            opts,
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
