//! Main entrypoint
// Launches the application by instantiating all components
use std::{
    collections::HashSet,
    net::IpAddr,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::{ArgAction, Parser};
use confy::load_path;
use minidsp::utils::OwnedJoinHandle;
use once_cell::sync::OnceCell;
use tokio::sync::RwLock;

use crate::{
    alsa_mixer::AlsaMixerManager, config::Config, device_manager::DeviceManager,
    discovery::Registry,
};

pub mod alsa_card_detect;
pub mod alsa_mixer;
pub mod alsa_softvol;
pub mod config;
pub mod device_manager;
pub mod discovery;
pub mod http;
pub mod tcp;

static APP: OnceCell<RwLock<App>> = OnceCell::new();

#[derive(Clone, Parser, Debug, Default)]
#[clap(version=env!("CARGO_PKG_VERSION"), author=env!("CARGO_PKG_AUTHORS"))]
pub struct Opts {
    /// Read config file from path
    #[clap(short, long)]
    config: Option<String>,

    /// Verbosity level. -v display decoded commands and responses -vv display decoded commands including readfloats -vvv display hex data frames
    #[clap(short, long, action = ArgAction::Count)]
    verbose: u8,

    /// Log commands and responses to a file
    #[clap(long, env = "MINIDSP_LOG")]
    log: Option<PathBuf>,

    /// Bind address for the TCP server component
    #[clap(default_value = "0.0.0.0:5333")]
    bind_address: String,

    /// If set, advertises the TCP component so it's discoverable from minidsp apps, using the given device name
    #[clap(long)]
    advertise: Option<String>,

    /// IP to use when advertising, required if --advertise is set
    #[clap(long)]
    ip: Option<String>,
}

pub struct App {
    opts: Opts,
    config: Config,
    #[allow(dead_code)]
    device_manager: Option<Arc<DeviceManager>>,
    /// ALSA mixer manager. Stored here to maintain the Arc reference;
    /// actual sync operations run in background task via cloned Arc.
    /// The manager is accessed through APP.get() in the sync task.
    #[allow(dead_code)]
    alsa_mixer: Option<Arc<AlsaMixerManager>>,
    #[allow(dead_code)]
    handles: Vec<OwnedJoinHandle<Result<(), anyhow::Error>>>,
}

impl App {
    pub fn new(opts: Opts, config: Config) -> RwLock<Self> {
        RwLock::new(Self {
            device_manager: None,
            alsa_mixer: None,
            handles: Vec::new(),
            opts,
            config,
        })
    }

    pub fn start(&mut self) {
        let registry = Registry::new();

        // If we're advertising a device, make sure to avoid discovering ourselves
        let our_ips: HashSet<IpAddr> = self
            .config
            .tcp_servers
            .iter()
            .filter_map(|s| {
                s.advertise
                    .as_ref()
                    .and_then(|a| IpAddr::from_str(&a.ip).ok())
            })
            .collect();

        let device_mgr = DeviceManager::new(registry, our_ips, self.config.ignore_advertisements);

        let http_server = self.config.http_server.clone();
        self.handles.push(
            tokio::spawn(async move {
                http::main(http_server).await?;
                Ok(())
            })
            .into(),
        );

        for server in &self.config.tcp_servers {
            let server = server.clone();
            self.handles.push(
                tokio::spawn(async move {
                    tcp::main(server).await?;
                    Ok(())
                })
                .into(),
            );
        }

        for static_device in &self.config.static_devices {
            device_mgr.register_static(&static_device.url);
        }

        let device_mgr = Arc::new(device_mgr);
        self.device_manager.replace(device_mgr.clone());

        // Initialize ALSA mixer on Linux
        #[cfg(target_os = "linux")]
        {
            let alsa_config = self.config.alsa_mixer.clone().unwrap_or_default();

            if alsa_config.enabled {
                let device_mgr_clone = device_mgr.clone();
                let alsa_config_clone = alsa_config.clone();

                self.handles.push(
                    tokio::spawn(async move {
                        // Give device manager time to discover devices
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                        // STEP 1: Get device product name
                        let device_product_name = device_mgr_clone.get_device(0)
                            .and_then(|device| {
                                device.device_spec().map(|spec| spec.product_name.to_string())
                            });

                        // STEP 2: Detect MiniDSP card
                        let detected_card_string = if let Some(ref product_name) = device_product_name {
                            crate::alsa_card_detect::detect_card_for_device(product_name)
                        } else {
                            log::warn!("No MiniDSP device detected at startup, skipping ALSA setup");
                            None
                        };

                        // STEP 3: Parse card number from detected card
                        let detected_card_number = detected_card_string.as_ref()
                            .and_then(|s| crate::alsa_card_detect::parse_card_number(s));

                        // STEP 4: Determine card to use (detected or config override)
                        let (card_to_use, card_number) = if let Some(num) = detected_card_number {
                            let card_str = format!("hw:{}", num);
                            log::info!("Using detected MiniDSP card: {}", card_str);
                            (card_str, num)
                        } else if let Some(ref config_card) = alsa_config_clone.card_name {
                            log::warn!("Device not detected, using configured card: {}", config_card);
                            let num = crate::alsa_card_detect::parse_card_number(config_card)
                                .unwrap_or(0);
                            (config_card.clone(), num)
                        } else {
                            log::warn!("No device detected and no card configured, skipping ALSA setup");
                            return Ok(());
                        };

                        // STEP 5: Determine output device (use detected card or config override)
                        let output_device = alsa_config_clone.output_device
                            .unwrap_or_else(|| {
                                log::info!("Using detected card for audio output: {}", card_to_use);
                                card_to_use.clone()
                            });

                        // STEP 6: Get control name
                        let control_name = alsa_config_clone.control_name
                            .unwrap_or_else(|| "Digital".to_string());

                        log::info!("ALSA configuration: card={}, output={}, control='{}'",
                                  card_to_use, output_device, control_name);

                        // STEP 7: Create softvol config if control doesn't exist
                        if !crate::alsa_softvol::check_softvol_exists(&card_to_use, &control_name) {
                            log::info!(
                                "ALSA control '{}' not found on {}, attempting to create softvol configuration",
                                control_name, card_to_use
                            );

                            if let Some(ref dev_name) = device_product_name {
                                match crate::alsa_softvol::write_user_asoundrc(
                                    dev_name,
                                    card_number,
                                    &control_name,
                                    &output_device,
                                    card_number,  // Control on detected card
                                ) {
                                    Ok(()) => {
                                        log::warn!("Created ALSA softvol configuration in ~/.asoundrc");
                                        log::warn!("  Control '{}' on card {}", control_name, card_number);
                                        log::warn!("  Audio output: {}", output_device);
                                        log::warn!("IMPORTANT: Run 'sudo alsactl init' or restart to activate");
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to create softvol config: {}", e);
                                    }
                                }
                            }
                        } else {
                            log::info!("Found existing ALSA control '{}'", control_name);
                        }

                        // STEP 8: Initialize mixer with detected card
                        let mut alsa_mixer = AlsaMixerManager::new(
                            Some(card_to_use.clone()),
                            Some(control_name.clone()),
                            alsa_config_clone.sync_interval_ms,
                        );

                        if let Err(e) = alsa_mixer.initialize() {
                            log::warn!("Failed to initialize ALSA mixer: {}", e);
                            log::info!("ALSA integration disabled due to initialization failure");
                        } else {
                            log::info!("ALSA mixer initialized successfully on {}", card_to_use);

                            if let Some(app) = APP.get() {
                                if let Ok(mut app_write) = app.try_write() {
                                    app_write.alsa_mixer.replace(Arc::new(alsa_mixer));
                                }
                            }
                        }
                        Ok(())
                    })
                    .into(),
                );
            } else {
                log::info!("ALSA mixer integration disabled in configuration");
            }
        }

        // Start ALSA sync task on Linux (delayed to allow ALSA init to complete)
        #[cfg(target_os = "linux")]
        {
            let alsa_config = self.config.alsa_mixer.clone().unwrap_or_default();
            if alsa_config.enabled {
                let device_mgr_clone = device_mgr.clone();

                self.handles.push(
                    tokio::spawn(async move {
                        // Wait for ALSA mixer to be initialized
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

                        if let Some(app) = APP.get() {
                            if let Ok(app_read) = app.try_read() {
                                if let Some(alsa_mixer) = &app_read.alsa_mixer {
                                    let mixer = alsa_mixer.clone();
                                    log::info!("ALSA mixer bidirectional sync enabled");

                                    if let Err(e) = crate::alsa_mixer::sync_task(mixer, device_mgr_clone).await {
                                        log::error!("ALSA sync task error: {}", e);
                                    }
                                }
                            }
                        }
                        Ok(())
                    })
                    .into(),
                );
            }
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

    let app = App::new(opts, config);
    APP.set(app).ok().unwrap();

    {
        let mut app_mut = APP.get().unwrap().try_write().unwrap();
        app_mut.start();
    }

    std::future::pending().await
}
