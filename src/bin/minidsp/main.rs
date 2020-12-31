//! MiniDSP Control Program

use anyhow::{anyhow, Result};
use bytes::Bytes;
use clap::Clap;
use debug::DebugCommands;
use minidsp::{
    device, discovery, server,
    transport::{net::NetTransport, Transport},
    Gain, MiniDSP, Source,
};
use std::{num::ParseIntError, str::FromStr, sync::Arc};
use tokio::net::TcpStream;
use tokio_stream::StreamExt;

mod debug;
mod handlers;

#[derive(Clap, Debug)]
#[clap(version = "1.1.0", author = "Mathieu Rene")]
struct Opts {
    /// The USB vendor and product id (2752:0011 for the 2x4HD)
    #[clap(name = "usb", long)]
    hid_option: Option<Option<ProductId>>,

    #[clap(name = "tcp", long)]
    /// The target address of the server component
    tcp_option: Option<String>,

    #[clap(subcommand)]
    subcmd: Option<SubCommand>,
}

#[derive(Clap, Debug)]
enum SubCommand {
    /// Set the master output gain [-127, 0]
    Gain { value: Gain },

    /// Set the master mute status
    Mute {
        #[clap(parse(try_from_str = on_or_off))]
        value: bool,
    },
    /// Set the active input source
    Source { value: Source },

    /// Set the current active configuration,
    Config { value: u8 },

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

    /// Look for existing devices on the network
    Discover,

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

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let opts: Opts = Opts::parse();

    if let Some(SubCommand::Discover) = opts.subcmd {
        use discovery::client::discover;
        let mut s = Box::new(discover().await?);
        while let Some(Ok((packet, _addr))) = s.next().await {
            println!("{:?}", packet);
        }
    }

    let transport: Arc<dyn Transport> = {
        if let Some(tcp) = opts.tcp_option {
            let stream = TcpStream::connect(tcp).await?;
            Arc::new(NetTransport::new(stream))
        } else if let Some(usb_options) = opts.hid_option {
            let mut vid: Option<u16> = None;
            let mut pid: Option<u16> = None;

            if let Some(addr) = usb_options {
                vid = Some(addr.vid);
                pid = addr.pid;
            }

            #[cfg(feature = "hid")]
            {
                use minidsp::transport::hid::find_minidsp;
                Arc::new(find_minidsp(vid, pid)?)
            }
            #[cfg(not(feature = "hid"))]
            {
                let _ = vid;
                let _ = pid;
                return Err(anyhow!("no transport configured"));
            }
        } else {
            return Err(anyhow!("no transport configured (use --usb or --tcp)"));
        }
    };

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
