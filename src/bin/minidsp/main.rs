//! MiniDSP Control Program

use anyhow::{anyhow, Result};
use bytes::Bytes;
use clap::Clap;
use debug::DebugCommands;
use minidsp::transport::net;
use minidsp::{
    device, discovery, server,
    transport::{net::NetTransport, Transport},
    Gain, MiniDSP,
};
use std::{num::ParseIntError, str::FromStr, sync::Arc};
use tokio::net::TcpStream;

mod cec;
mod debug;
mod handlers;

#[cfg(feature = "hid")]
use minidsp::transport::hid;
use minidsp::transport::Openable;
use std::time::Duration;

#[derive(Clap, Debug)]
#[clap(version=env!("CARGO_PKG_VERSION"), author=env!("CARGO_PKG_AUTHORS"))]
struct Opts {
    /// The USB vendor and product id (2752:0011 for the 2x4HD)
    #[clap(name = "usb", env = "MINIDSP_USB", long)]
    #[cfg(feature = "hid")]
    hid_option: Option<hid::Device>,

    #[clap(name = "tcp", env = "MINIDSP_TCP", long)]
    /// The target address of the server component
    tcp_option: Option<String>,

    #[clap(subcommand)]
    subcmd: Option<SubCommand>,
}

#[derive(Clap, Debug)]
enum SubCommand {
    /// Try to find reachable devices
    Probe,

    /// Set the master output gain [-127, 0]
    Gain {
        value: Gain,
    },

    /// Set the master mute status
    Mute {
        #[clap(parse(try_from_str = on_or_off))]
        value: bool,
    },
    /// Set the active input source
    Source {
        value: String,
    },

    /// Set the current active configuration,
    Config {
        value: u8,
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

    /// Launch a server usable with `--tcp`, the mobile application, and the official client
    Server {
        #[clap(default_value = "0.0.0.0:5333")]
        bind_address: String,
        #[clap(long)]
        advertise: Option<String>,
        #[clap(long)]
        ip: Option<String>,
    },

    Cec,

    /// Low-level debug utilities
    Debug(DebugCommands),
}

#[derive(Clap, Debug)]
enum InputCommand {
    /// Set the input gain for this channel
    Gain {
        /// Gain in dB
        value: Gain,
    },

    /// Set the master mute status
    Mute {
        #[clap(parse(try_from_str = on_or_off))]
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
        /// Parametric EQ index
        index: usize,

        #[clap(subcommand)]
        cmd: PEQCommand,
    },
}

#[derive(Clap, Debug)]
enum RoutingCommand {
    /// Controls whether the output matrix for this input is enabled for the given output index
    Enable {
        #[clap(parse(try_from_str = on_or_off))]
        /// Whether this input is enabled for the given output channel
        value: bool,
    },
    Gain {
        /// Output gain in dB
        value: Gain,
    },
}

#[derive(Clap, Debug)]
enum OutputCommand {
    /// Set the input gain for this channel
    Gain {
        /// Output gain in dB
        value: Gain,
    },

    /// Set the master mute status
    Mute {
        #[clap(parse(try_from_str = on_or_off))]
        value: bool,
    },

    /// Set the delay associated to this channel
    Delay {
        /// Delay in milliseconds
        delay: f32,
    },

    /// Set phase inversion on this channel
    Invert {
        #[clap(parse(try_from_str = on_or_off))]
        value: bool,
    },

    /// Control the parametric equalizer
    PEQ {
        /// Parametric EQ index
        index: usize,

        #[clap(subcommand)]
        cmd: PEQCommand,
    },
}

#[derive(Clap, Debug)]
enum PEQCommand {
    /// Set biquad coefficients
    Set {
        /// Biquad coefficients
        coeff: Vec<f32>,
    },

    /// Sets the bypass toggle
    Bypass {
        #[clap(parse(try_from_str = on_or_off))]
        value: bool,
    },
}

#[derive(Debug, Clap)]
pub struct ProductId {
    pub vid: u16,
    pub pid: Option<u16>,
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

async fn get_transport(opts: &Opts) -> Result<Arc<dyn Transport>> {
    if let Some(tcp) = &opts.tcp_option {
        let stream = TcpStream::connect(tcp).await?;
        return Ok(Arc::new(NetTransport::new(stream)));
    }

    #[cfg(feature = "hid")]
    {
        if let Some(device) = &opts.hid_option {
            return Ok(Arc::new(device.open().await?));
        }

        // If no device was passed, do a best effort to figure out the right device to open
        let hid_devices = hid::discover()?;
        if hid_devices.len() == 1 {
            return Ok(Arc::new(hid_devices[0].open().await?));
        } else if !hid_devices.is_empty() {
            eprintln!("There are multiple potential devices, use --usb path=... to disambiguate");
            for device in &hid_devices {
                eprintln!("{}", device)
            }
            return Err(anyhow!("Multiple candidate usb devices are detected."));
        }
    }

    return Err(anyhow!("Couldn't find any MiniDSP devices"));
}

async fn run_probe() -> Result<()> {
    #[cfg(feature = "hid")]
    {
        // Probe for local usb devices
        let devices = hid::discover()?;
        if devices.is_empty() {
            println!("No matching local USB devices detected.")
        } else {
            for device in &devices {
                println!("Found: {}", device);
            }
        }
    }

    println!("Probing for network devices...");
    let devices = net::discover(Duration::from_secs(2)).await?;
    if devices.is_empty() {
        println!("No network devices detected")
    } else {
        for device in &devices {
            println!("Found: {}", device);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let opts: Opts = Opts::parse();

    if let Some(SubCommand::Probe) = opts.subcmd {
        run_probe().await?;
        return Ok(());
    }

    let transport: Arc<dyn Transport> = get_transport(&opts).await?;

    let device = MiniDSP::new(transport, &device::DEVICE_2X4HD);
    handlers::run_command(&device, opts.subcmd).await?;

    // Always output the current master status and input/output levels
    let master_status = device.get_master_status().await?;
    println!("{:?}", master_status);

    let input_levels = device.get_input_levels().await?;
    let strs: Vec<String> = input_levels.iter().map(|x| format!("{:.1}", *x)).collect();
    println!("Input levels: {}", strs.join(", "));

    let output_levels = device.get_output_levels().await?;
    let strs: Vec<String> = output_levels.iter().map(|x| format!("{:.1}", *x)).collect();
    println!("Output levels: {}", strs.join(", "));

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
    Ok(Bytes::from(hex::decode(s.replace(" ", ""))?))
}

fn parse_hex_u16(src: &str) -> Result<u16, ParseIntError> {
    u16::from_str_radix(src, 16)
}
