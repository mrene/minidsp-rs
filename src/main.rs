use anyhow::{anyhow, Result};
use bytes::Bytes;
use clap::Clap;
use minidsp::commands::{roundtrip, CustomUnaryCommand, ReadFloats, ReadMemory};
use minidsp::transport::net::NetTransport;
use minidsp::transport::Transport;
use minidsp::{device, discovery};
use minidsp::{server, Gain, MiniDSP, Source};
use std::net::Ipv4Addr;
use std::num::ParseIntError;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;

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
        value: Source,
    },

    Server {
        #[clap(default_value = "0.0.0.0:5333")]
        bind_address: String,
        #[clap(long)]
        advertise: Option<String>,
        #[clap(long)]
        ip: Option<String>,
    },
    Discover,
    Debug(DebugCommands),
}

#[derive(Clap, Debug)]
enum DebugCommands {
    /// Send a hex-encoded command
    Send {
        #[clap(parse(try_from_str = parse_hex))]
        value: Bytes,
        #[clap(long, short)]
        watch: bool,
    },

    /// Dumps memory starting at a given address
    Dump {
        #[clap(parse(try_from_str = parse_hex_u16))]
        addr: u16,
    },

    /// Dumps contiguous float data starting at a given address
    DumpFloat {
        #[clap(parse(try_from_str = parse_hex_u16))]
        addr: u16,
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

    match opts.subcmd {
        Some(SubCommand::Gain { value }) => device.set_master_volume(value).await?,
        Some(SubCommand::Mute { value }) => device.set_master_mute(value).await?,
        Some(SubCommand::Source { value }) => device.set_source(value).await?,
        Some(SubCommand::Server {
            bind_address,
            advertise,
            ip,
        }) => {
            if let Some(hostname) = advertise {
                let mut packet = discovery::DiscoveryPacket {
                    mac_address: [10, 20, 30, 40, 50, 60],
                    ip_address: Ipv4Addr::new(192, 168, 1, 33),
                    hwid: 0,
                    typ: 0,
                    sn: 0,
                    hostname,
                };
                if let Some(ip) = ip {
                    packet.ip_address = Ipv4Addr::from_str(ip.as_str())?;
                }
                let interval = tokio::time::Duration::from_secs(1);
                tokio::spawn(discovery::server::advertise_packet(packet, interval));
            }
            server::serve(bind_address, device.transport.clone()).await?
        }
        // Handled earlier
        Some(SubCommand::Discover) => return Ok(()),

        Some(SubCommand::Debug(debug)) => {
            match debug {
                DebugCommands::Send { value, watch } => {
                    let response =
                        roundtrip(device.transport.as_ref(), CustomUnaryCommand::new(value))
                            .await?;
                    println!("response: {:02x?}", response.as_ref());
                    let mut sub = device.transport.subscribe();
                    if watch {
                        // Print out all received packets
                        while let Ok(packet) = sub.recv().await {
                            println!("> {:02x?}", packet.as_ref());
                        }
                    }
                }
                DebugCommands::Dump { addr } => {
                    let view =
                        roundtrip(device.transport.as_ref(), ReadMemory { addr, size: 60 }).await?;

                    use hexplay::HexViewBuilder;
                    let view = HexViewBuilder::new(view.data.as_ref())
                        .address_offset(view.base as usize)
                        .row_width(16)
                        .finish();
                    view.print().unwrap();
                }

                DebugCommands::DumpFloat { addr } => {
                    let len = 14;
                    let view = roundtrip(
                        device.transport.as_ref(),
                        ReadFloats {
                            addr,
                            len: len as u8,
                        },
                    )
                    .await?;
                    for i in addr..(addr + len) {
                        let val = view.get(i);
                        println!("{:04x?}: {:?}", i, val);
                    }
                }
            }
            return Ok(());
        }

        None => {}
    }

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
