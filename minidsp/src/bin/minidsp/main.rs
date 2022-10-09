//! MiniDSP Control Program

#![allow(clippy::upper_case_acronyms)]

use std::{
    fmt,
    fs::File,
    io::{BufRead, BufReader},
    num::ParseIntError,
    path::PathBuf,
    str::FromStr,
};

use anyhow::{anyhow, Result};
use bytes::Bytes;
use clap::{ArgAction, Parser};
use debug::DebugCommands;
use futures::{stream, StreamExt};
use handlers::run_server;
use minidsp::{
    builder::{Builder, DeviceHandle},
    device::DeviceKind,
    tcp_server,
    transport::net::{self, discovery},
    Gain, MiniDSP, MiniDSPError, Source,
};

mod debug;
mod handlers;

use std::{io::Read, time::Duration};

#[cfg(feature = "hid")]
use minidsp::transport::hid;

#[derive(Clone, Parser, Debug)]
#[clap(version=env!("CARGO_PKG_VERSION"), author=env!("CARGO_PKG_AUTHORS"))]
struct Opts {
    /// Verbosity level. -v display decoded commands and responses -vv display decoded commands including readfloats -vvv display hex data frames
    #[clap(short, long, action = ArgAction::Count)]
    verbose: u8,

    /// Output response format (text (default), json, jsonline)
    #[clap(long = "output", short = 'o', default_value = "text")]
    output_format: OutputFormat,

    #[clap(long, env = "MINIDSP_LOG")]
    /// Log commands and responses to a file
    log: Option<PathBuf>,

    /// Apply the given commands to all matching local usb devices
    #[clap(long)]
    all_local_devices: bool,

    /// Use the given device 0-based index (use minidsp probe for a list of available devices)
    #[clap(name = "device-index", env = "MINIDSP_INDEX", long, short = 'd')]
    device_index: Option<usize>,

    /// The USB vendor and product id (2752:0011 for the 2x4HD)
    #[clap(name = "usb", env = "MINIDSP_USB", long)]
    #[cfg(feature = "hid")]
    hid_option: Option<hid::Device>,

    #[clap(name = "tcp", env = "MINIDSP_TCP", long)]
    /// The target address of the server component
    tcp_option: Option<String>,

    /// Force the device to a specific product instead of probing its hardware id. May break things, use at your own risk.
    #[clap(name = "force-kind", long)]
    force_kind: Option<DeviceKind>,

    /// Directly connect to this transport url
    #[clap(long, env = "MINIDSP_URL")]
    url: Option<String>,

    /// Discover devices that are managed by the remote instance of minidspd
    #[clap(long, env = "MINIDSPD_URL")]
    daemon_url: Option<String>,

    /// Discover devices that are managed by the local instance of minidspd
    #[clap(long, env = "MINIDSP_SOCK")]
    #[cfg(target_family = "unix")]
    daemon_sock: Option<String>,

    #[clap(short = 'f')]
    /// Read commands to run from the given filename (use - for stdin)
    file: Option<PathBuf>,

    #[clap(subcommand)]
    subcmd: Option<SubCommand>,
}

impl Opts {
    // Applies transport and logging options to this builder
    async fn apply_builder(&self, builder: &mut Builder) -> Result<(), MiniDSPError> {
        #[allow(unused_mut)]
        let mut bound = false;
        #[cfg(target_family = "unix")]
        if let Some(socket_path) = &self.daemon_sock {
            builder.with_unix_socket(socket_path).await?;
            bound = true;
        }

        if bound {
        } else if let Some(tcp) = &self.tcp_option {
            let tcp = if tcp.contains(':') {
                tcp.to_string()
            } else {
                format!("{}:5333", tcp)
            };
            builder.with_tcp(tcp).unwrap();
        } else if let Some(url) = &self.url {
            builder
                .with_url(url)
                .map_err(|_| MiniDSPError::InvalidURL)?;
        } else if let Some(url) = &self.daemon_url {
            builder.with_http(url).await?;
        } else if let Some(device) = self.hid_option.as_ref() {
            if let Some(ref path) = device.path {
                builder.with_usb_path(path);
            } else if let Some((vid, pid)) = device.id {
                builder.with_usb_product_id(vid, pid)?;
            }
        } else {
            #[cfg(target_family = "unix")]
            let _ = builder.with_unix_socket("/tmp/minidsp.sock").await;
            let _ = builder.with_default_usb();
        }

        builder.with_logging(self.verbose as u8, self.log.clone());

        if let Some(force_kind) = self.force_kind {
            builder.force_device_kind(force_kind);
        }

        Ok(())
    }
}

#[derive(Clone, Parser, Debug)]
enum SubCommand {
    /// Try to find reachable devices
    Probe {
        #[clap(long, short)]
        /// Listen for advertisement broadcasts from WI-DGs
        net: bool,
    },

    /// Prints the master status and current levels
    Status,

    /// Set the master output gain [-127, 0]
    Gain {
        #[clap(long, short)]
        /// Specify the value as a relative increment on top of the current gain.
        relative: bool,

        /// Gain in decibels, between -127 and 0 inclusively
        value: Gain,
    },

    /// Set the master mute status
    Mute {
        /// on, off, toggle
        value: ToggleBool,
    },
    /// Set the active input source
    Source {
        /// The source to use:
        /// analog, toslink, spdif, usb, aesebu, rca, xlr, lan, i2s, bluetooth
        value: Source,
    },

    /// Set the current active configuration,
    Config {
        /// 0-indexed configuation preset value (0, 1, 2, 3)
        value: u8,
    },

    /// Sets whether Dirac Live is enabled
    Dirac {
        /// on, off, toggle
        value: ToggleBool,
    },

    /// Control settings regarding input channels
    Input {
        /// Index of the input channel, starting at 0
        input_index: usize,

        #[clap(subcommand)]
        cmd: InputCommand,
    },

    /// Control settings regarding output channels
    Output {
        /// Index of the output channel, starting at 0
        output_index: usize,

        #[clap(subcommand)]
        cmd: OutputCommand,
    },

    /// (deprecated) Launch a server usable with `--tcp`, the mobile application, and the official client
    Server {
        #[clap(default_value = "0.0.0.0:5333")]
        bind_address: String,
        #[clap(long)]
        advertise: Option<String>,
        #[clap(long)]
        ip: Option<String>,
    },

    /// Low-level debug utilities
    Debug {
        #[clap(subcommand)]
        cmd: DebugCommands,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum ToggleBool {
    On,
    Off,
    Toggle,
}

impl FromStr for ToggleBool {
    type Err = &'static str;

    fn from_str(mute: &str) -> Result<Self, Self::Err> {
        match mute {
            "on" => Ok(ToggleBool::On),
            "true" => Ok(ToggleBool::On),
            "off" => Ok(ToggleBool::Off),
            "false" => Ok(ToggleBool::Off),
            "toggle" => Ok(ToggleBool::Toggle),
            _ => Err("expected `on`, `true`, `off`, `false`, `toggle`"),
        }
    }
}

#[derive(Clone, Parser, Debug)]
enum InputCommand {
    /// Set the input gain for this channel
    Gain {
        /// Gain in dB
        value: Gain,
    },

    /// Set the master mute status
    Mute {
        #[clap(value_parser = on_or_off)]
        /// on | off
        value: bool,
    },

    /// Controls signal routing from this input
    Routing {
        /// Index of the output channel starting at 0
        output_index: usize,

        #[clap(subcommand)]
        cmd: RoutingCommand,
    },

    /// Control the parametric equalizer
    PEQ {
        /// Parametric EQ index (all | <id>) (0 to 9 inclusively)
        index: PEQTarget,

        #[clap(subcommand)]
        cmd: FilterCommand,
    },
}

#[derive(Clone, Parser, Debug)]
enum RoutingCommand {
    /// Controls whether the output matrix for this input is enabled for the given output index
    Enable {
        #[clap(value_parser = on_or_off)]
        /// Whether this input is enabled for the given output channel
        value: bool,
    },
    Gain {
        /// Output gain in dB
        value: Gain,
    },
}

#[derive(Clone, Parser, Debug)]
enum OutputCommand {
    /// Set the output gain for this channel
    Gain {
        /// Output gain in dB
        value: Gain,
    },

    /// Set the master mute status
    Mute {
        /// on | off
        #[clap(value_parser = on_or_off)]
        value: bool,
    },

    /// Set the delay associated to this channel
    Delay {
        /// Delay in milliseconds
        delay: f32,
    },

    /// Set phase inversion on this channel
    Invert {
        #[clap(value_parser = on_or_off)]
        /// on | off
        value: bool,
    },

    /// Control the parametric equalizer
    PEQ {
        /// Parametric EQ index (all | <id>) (0 to 9 inclusively)
        index: PEQTarget,

        #[clap(subcommand)]
        cmd: FilterCommand,
    },

    /// Control the FIR filter
    FIR {
        #[clap(subcommand)]
        cmd: FilterCommand,
    },

    /// Control crossovers (2x 4 biquads)
    Crossover {
        /// Group index (0 or 1)
        group: usize,

        /// Filter index (all | 0 | 1 | 3)
        index: PEQTarget,

        #[clap(subcommand)]
        cmd: FilterCommand,
    },

    /// Control the compressor
    Compressor {
        /// Bypasses the compressor (on | off)
        #[clap(short='b', long, value_parser = on_or_off)]
        bypass: Option<bool>,

        /// Sets the threshold in dBFS
        #[clap(short = 't', long, allow_hyphen_values(true))]
        threshold: Option<f32>,

        /// Sets the ratio
        #[clap(short = 'k', long)]
        ratio: Option<f32>,

        /// Sets the attack time in ms
        #[clap(short = 'a', long)]
        attack: Option<f32>,

        /// Sets the release time in ms
        #[clap(short = 'r', long)]
        release: Option<f32>,
    },
}

#[derive(Debug, Clone, Copy)]
enum PEQTarget {
    All,
    One(usize),
}

impl FromStr for PEQTarget {
    type Err = <usize as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.to_lowercase() == "all" {
            Ok(PEQTarget::All)
        } else {
            Ok(PEQTarget::One(usize::from_str(s)?))
        }
    }
}

#[derive(Clone, Parser, Debug)]
enum FilterCommand {
    /// Set coefficients
    Set {
        /// Coefficients
        coeff: Vec<f32>,
    },

    /// Sets the bypass toggle
    Bypass {
        #[clap(value_parser = on_or_off)]
        /// on | off
        value: bool,
    },

    /// Sets all coefficients back to their default values and un-bypass them
    Clear,

    /// Imports the coefficients from the given file
    Import {
        /// Filename containing the coefficients in REW format
        filename: PathBuf,
        /// Import file format
        format: Option<String>,
    },
}

#[derive(Debug, Parser)]
pub struct ProductId {
    pub vid: u16,
    pub pid: Option<u16>,
}

#[derive(Debug, strum::EnumString, strum::Display, Clone, Copy, Eq, PartialEq)]
#[strum(serialize_all = "lowercase")]
pub enum OutputFormat {
    Text,
    Json,
    JsonLine,
}

impl OutputFormat {
    pub fn format<T>(self, obj: &T) -> String
    where
        T: serde::Serialize + fmt::Display,
    {
        match self {
            OutputFormat::Text => format!("{}", obj),
            OutputFormat::Json => {
                serde_json::to_string_pretty(obj).expect("couldn't serialize object as json")
            }
            OutputFormat::JsonLine => {
                serde_json::to_string(obj).expect("couldn't serialize object as json")
            }
        }
    }
}

impl FromStr for ProductId {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.split(':').collect();
        if parts.len() > 2 {
            return Err("");
        }

        let vid = u16::from_str_radix(parts[0], 16).map_err(|_| "couldn't parse vendor id")?;
        let mut pid: Option<u16> = None;
        if parts.len() > 1 {
            pid = Some(u16::from_str_radix(parts[1], 16).map_err(|_| "couldn't parse product id")?);
        }

        Ok(ProductId { vid, pid })
    }
}

async fn run_probe(devices: Vec<DeviceHandle>, net: bool) -> Result<()> {
    for (index, dev) in devices.iter().enumerate() {
        println!(
            "{}: Found {} with serial {} at {} [hw_id: {}, dsp_version: {}]",
            index,
            dev.device_spec.product_name,
            dev.device_info.serial,
            dev.url,
            dev.device_info.hw_id,
            dev.device_info.dsp_version
        );
    }

    if net {
        println!("Probing for network devices...");
        let devices = net::discover_timeout(Duration::from_secs(8)).await?;
        if devices.is_empty() {
            println!("No network devices detected")
        } else {
            for device in &devices {
                println!("Found: {}", device);
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder().format_timestamp_millis().init();
    let opts: Opts = Opts::parse();
    let mut builder = Builder::new();
    opts.apply_builder(&mut builder).await?;

    let mut devices: Vec<_> = builder
        .probe()
        .filter_map(|x| async move { x.ok() })
        .collect()
        .await;

    devices.sort_by(|a, b| a.device_info.serial.cmp(&b.device_info.serial));

    if let Some(SubCommand::Probe { net }) = opts.subcmd {
        run_probe(devices, net).await?;
        return Ok(());
    }

    if !opts.all_local_devices {
        if let Some(index) = opts.device_index {
            let dev = devices.remove(index);
            devices.clear();
            devices.push(dev);
        }
        devices.truncate(1);
    }

    if let Some(SubCommand::Server { .. }) = opts.subcmd {
        log::warn!("The `server` command is deprecated and will be removed in a future release. Use `minidspd` instead.");

        let transport = devices
            .first()
            .ok_or_else(|| anyhow!("No devices found"))?
            .transport
            .try_clone()
            .expect("device has disappeared");

        run_server(opts.subcmd.unwrap(), Box::pin(transport)).await?;
        return Ok(());
    }

    let devices: Vec<_> = devices
        .into_iter()
        .map(|dev| dev.to_minidsp().expect("device has disappeared"))
        .collect();

    if devices.is_empty() {
        return Err(anyhow!("No devices found"));
    }

    match &opts.file {
        Some(filename) => {
            let file: Box<dyn Read> = {
                if filename.to_string_lossy() == "-" {
                    Box::new(std::io::stdin())
                } else {
                    Box::new(File::open(filename)?)
                }
            };
            let reader = BufReader::new(file);
            let cmds = reader.lines().filter_map(|s| {
                let trimmed = s.ok()?.trim().to_string();
                if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    Some(trimmed)
                } else {
                    None
                }
            });

            for cmd in cmds {
                let words = shellwords::split(&cmd)?;
                let prefix = &["minidsp".to_string()];
                let words = prefix.iter().chain(words.iter());
                let this_opts = Opts::try_parse_from(words);
                let this_opts = match this_opts {
                    Ok(x) => x,
                    Err(e) => {
                        eprintln!("While executing: {}\n{}", cmd, e);
                        return Err(anyhow!("Command failure"));
                    }
                };

                // Run this command on all devices in parallel
                let results = stream::iter(&devices)
                    .then(|dev| handlers::run_command(dev, this_opts.subcmd.as_ref(), &opts))
                    .collect::<Vec<_>>()
                    .await;
                for result in results {
                    result?;
                }
            }
        }
        None => {
            let results = stream::iter(&devices)
                .then(|dev| handlers::run_command(dev, opts.subcmd.as_ref(), &opts))
                .collect::<Vec<_>>()
                .await;
            for result in results {
                result?;
            }
        }
    }

    Ok(())
}

fn on_or_off(s: &str) -> Result<bool, &'static str> {
    match s {
        "on" => Ok(true),
        "true" => Ok(true),
        "off" => Ok(false),
        "false" => Ok(false),
        _ => Err("expected `on`, `true`, `off`, `false`"),
    }
}

fn parse_hex(s: &str) -> Result<Bytes, hex::FromHexError> {
    Ok(Bytes::from(hex::decode(s.replace(' ', ""))?))
}

fn parse_hex_u16(src: &str) -> Result<u16, ParseIntError> {
    u16::from_str_radix(src, 16)
}
