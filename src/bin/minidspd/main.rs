///! Main entrypoint
/// Launches the application by instantiating all components
///
use anyhow::{Context, Result};
use clap::Clap;
use config::Config;
use confy::load_path;
use discovery::{DiscoveryEvent, Registry};
use minidsp::utils::OwnedJoinHandle;
use once_cell::sync::OnceCell;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use tokio::sync::RwLock;

mod config;
mod device_manager;
mod discovery;
mod http;
mod logging;
mod tcp;

static APP: OnceCell<RwLock<App>> = OnceCell::new();

#[derive(Clone, Clap, Debug, Default)]
#[clap(version=env!("CARGO_PKG_VERSION"), author=env!("CARGO_PKG_AUTHORS"))]
pub struct Opts {
    /// Read config file from path
    #[clap(short, long)]
    config: Option<String>,

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
    pub fn start(opts: Opts, config: Config) -> Self {
        let registry = Registry::new();

        // If we're advertising a device, make sure to avoid discovering ourselves
        let this_ip = opts
            .ip
            .as_ref()
            .and_then(|ip| std::net::IpAddr::from_str(ip.as_str()).ok());

        let device_mgr = device_manager::DeviceManager::new(registry, this_ip);
        let mut handles = vec![];

        let http_server = config.http_server.clone();
        handles.push(
            tokio::spawn(async move {
                http::main(http_server).await?;
                Ok(())
            })
            .into(),
        );

        for server in &config.tcp_servers {
            let server = server.clone();
            handles.push(
                tokio::spawn(async move {
                    tcp::main(server).await?;
                    Ok(())
                })
                .into(),
            );
        }

        App {
            device_manager: device_mgr,
            handles,
            opts,
        }
    }

    fn load_config(path: Option<impl AsRef<Path>>) -> Result<Config, confy::ConfyError> {
        match path {
            None => Ok(Config::default()),
            Some(path) => load_path(path),
        }
    }
}
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let opts: Opts = Opts::parse();
    let config: Config =
        App::load_config(opts.config.as_ref()).context("cannot load configuration file")?;

    let app = App::start(opts, config);
    APP.set(app.into()).ok().unwrap();

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
