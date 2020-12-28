use anyhow::{anyhow, Result};
use bytes::Bytes;
use clap::Clap;
use minidsp::commands::{roundtrip, CustomUnaryCommand};
use minidsp::transport::net::NetTransport;
use minidsp::transport::Transport;
use minidsp::{server, Gain, MiniDSP, Source};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpStream;

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
    /// Send a debug command (hex-encoded)
    Send {
        #[clap(parse(try_from_str = parse_hex))]
        value: Bytes,
    },
    Server {
        #[clap(default_value = "0.0.0.0:5333")]
        bind_address: String,
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

        let vid = u16::from_str_radix(parts[0], 16).map_err(|_| "coudln't parse vendor id")?;
        let mut pid: Option<u16> = None;
        if parts.len() > 1 {
            pid = Some(u16::from_str_radix(parts[1], 16).map_err(|_| "couldn't parse product id")?);
        }

        Ok(ProductId { vid, pid })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

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

    let device = MiniDSP::new(transport);

    match opts.subcmd {
        Some(SubCommand::Gain { value }) => device.set_master_volume(value).await?,
        Some(SubCommand::Mute { value }) => device.set_master_mute(value).await?,
        Some(SubCommand::Source { value }) => device.set_source(value).await?,
        Some(SubCommand::Send { value }) => {
            let response =
                roundtrip(device.transport.as_ref(), CustomUnaryCommand::new(value)).await?;
            println!("response: {:02x?}", response.as_ref());
        }
        Some(SubCommand::Server { bind_address }) => {
            server::serve(bind_address, device.transport.clone()).await?
        }
        None => {}
    }

    let master_status = device.get_master_status().await?;
    println!("{:?}", master_status);

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
