//! This contain command line utilities for debugging and inspecting lower level protocol commands
use anyhow::Result;
use bytes::Bytes;
use clap::Clap;

use super::{parse_hex, parse_hex_u16};
use minidsp::MiniDSP;
use minidsp::{
    commands::{
        roundtrip, CustomUnaryCommand, ExtendView, FloatView, MemoryView, ReadFloats,
        ReadHardwareId, ReadMemory,
    },
    source,
};
use std::ops::Deref;

pub(crate) async fn run_debug(device: &MiniDSP<'_>, debug: DebugCommands) -> Result<()> {
    match debug {
        DebugCommands::Send { value, watch } => {
            let response =
                roundtrip(device.transport.as_ref(), CustomUnaryCommand::new(value)).await?;
            println!("response: {:02x?}", response.as_ref());
            if watch {
                let mut sub = device.transport.subscribe();
                // Print out all received packets
                while let Ok(packet) = sub.recv().await {
                    println!("> {:02x?}", packet.as_ref());
                }
            }
        }
        DebugCommands::Dump { addr, end_addr } => {
            let mut view = MemoryView {
                base: addr,
                data: Default::default(),
            };
            let end_addr = end_addr.unwrap_or(59);
            for i in (addr..end_addr).step_by(59) {
                let v =
                    roundtrip(device.transport.as_ref(), ReadMemory { addr: i, size: 59 }).await?;
                view.extend_with(v)?;
            }
            println!("\n");
            dump_memory(&view);
        }
        DebugCommands::DumpFloat { addr, end_addr } => {
            let len = 14;
            let end_addr = end_addr.unwrap_or(14);
            for i in (addr..end_addr).step_by(14) {
                let view = roundtrip(
                    device.transport.as_ref(),
                    ReadFloats {
                        addr: i,
                        len: len as u8,
                    },
                )
                .await?;
                dump_floats(&view);
            }
        }

        DebugCommands::Id => {
            #[cfg(feature = "hid")]
            {
                use minidsp::transport::hid;
                // Probe for local usb devices
                println!("Probing local hid devices:");
                let devices = hid::discover(hid::initialize_api()?.deref())?;
                if devices.is_empty() {
                    println!("No matching local USB devices detected.")
                } else {
                    for device in &devices {
                        println!("Found: {}", device);
                    }
                }
                println!()
            }

            let id = roundtrip(device.transport.as_ref(), ReadHardwareId {}).await?;
            println!("HW ID Response: {:02x?}", id.as_ref());

            let device_info = device.get_device_info().await?;
            println!(
                "HW ID: {}\nDSP Version: {}",
                device_info.hw_id, device_info.dsp_version
            );

            let sources = source::Source::mapping(&device_info);
            println!("Detected sources: {:?}", sources);

            println!("\nDumping memory:");
            let mut view =
                roundtrip(device.transport.as_ref(), ReadMemory::new(0xffa0, 59)).await?;
            view.extend_with(
                roundtrip(device.transport.as_ref(), ReadMemory::new(0xffa0 + 59, 59)).await?,
            )?;
            dump_memory(&view);

            println!("\n\nDumping readable floats:");
            for addr in (0x00..0xff).step_by(14) {
                let floats =
                    roundtrip(device.transport.as_ref(), ReadFloats::new(addr, 14)).await?;
                dump_floats(&floats);
            }
        }
    }

    std::process::exit(0);
}

fn dump_memory(view: &MemoryView) {
    use hexplay::HexViewBuilder;
    println!("len={:?}", view.data.len());
    let view = HexViewBuilder::new(view.data.as_ref())
        .address_offset(view.base as usize)
        .row_width(16)
        .finish();
    view.print().unwrap();
}

fn dump_floats(view: &FloatView) {
    for i in view.base..(view.base + view.data.len() as u16) {
        let val = view.get(i);
        if val != 0. {
            println!("{:04x?}: {:?}", i, val);
        }
    }
}

#[derive(Clap, Debug)]
pub enum DebugCommands {
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
        #[clap(parse(try_from_str = parse_hex_u16))]
        end_addr: Option<u16>,
    },

    /// Dumps contiguous float data starting at a given address
    DumpFloat {
        #[clap(parse(try_from_str = parse_hex_u16))]
        addr: u16,
        #[clap(parse(try_from_str = parse_hex_u16))]
        end_addr: Option<u16>,
    },

    /// Retrieves information about the device's identify
    Id,
}
