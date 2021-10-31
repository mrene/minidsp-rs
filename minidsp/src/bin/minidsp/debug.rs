//! This contain command line utilities for debugging and inspecting lower level protocol commands

use std::convert::TryInto;

use anyhow::Result;
use bytes::Bytes;
use clap::Parser;
use minidsp::{
    commands::{BytesWrap, Commands, ExtendView, FloatView, MemoryView},
    eeprom, source, MiniDSP,
};

use super::{parse_hex, parse_hex_u16};

pub(crate) async fn run_debug(device: &MiniDSP<'_>, debug: &DebugCommands) -> Result<()> {
    match debug {
        DebugCommands::Send { value } => {
            let response = device
                .client
                .roundtrip(Commands::Unknown {
                    cmd_id: value[0],
                    payload: BytesWrap(value.slice(1..)),
                })
                .await?;

            println!("response: {:02x?}", response);
        }
        &DebugCommands::Dump { addr, end_addr } => {
            let mut view = MemoryView {
                base: addr,
                data: Default::default(),
            };
            let end_addr = end_addr.unwrap_or(59);
            for i in (addr..end_addr).step_by(59) {
                view.extend_with(device.client.read_memory(i, 59).await?)?;
            }
            println!("\n");
            dump_memory(&view);
        }
        &DebugCommands::DumpFloat { addr, end_addr } => {
            let len = 14;
            let end_addr = end_addr.unwrap_or(14);
            for i in (addr..end_addr).step_by(14) {
                let view = device.client.read_floats(i, len as u8).await?;
                dump_floats(&view);
            }
        }
        DebugCommands::Id => {
            #[cfg(feature = "hid")]
            {
                use minidsp::transport::hid;
                // Probe for local usb devices
                println!("Probing local hid devices:");
                let api = hid::initialize_api()?;
                let mut api = api.lock().unwrap();
                let devices = hid::discover(&mut api)?;
                if devices.is_empty() {
                    println!("No matching local USB devices detected.")
                } else {
                    for device in &devices {
                        println!("Found: {}", device);
                    }
                }
                println!()
            }

            let device_info = device.get_device_info().await?;
            println!(
                "HW ID: {}\nDSP Version: {}",
                device_info.hw_id, device_info.dsp_version
            );

            let sources = source::Source::mapping(&device_info);
            println!("Detected sources: {:?}", sources);

            println!("\nDumping memory:");
            let mut view = device.client.read_memory(0xffa0, 59).await?;

            view.extend_with(device.client.read_memory(0xffa0 + 59, 59).await?)?;
            dump_memory(&view);

            println!("\n\nDumping readable floats:");
            for addr in (0x00..0xff).step_by(14) {
                let floats = device.client.read_floats(addr, 14).await?;
                dump_floats(&floats);
            }
        }
        &DebugCommands::SetSerial { value } => {
            if !(900000..=965535).contains(&900000) {
                return Err(anyhow::anyhow!("Serial must be between 900000 and 965535"));
            }
            let value: u16 = (value - 900000).try_into().unwrap();
            device
                .client
                .write_u16(eeprom::SERIAL_SHORT, value)
                .await?;
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

#[derive(Clone, Parser, Debug)]
pub enum DebugCommands {
    /// Send a hex-encoded command
    Send {
        #[clap(parse(try_from_str = parse_hex))]
        value: Bytes,
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

    /// Sets the device's serial number
    SetSerial { value: u32 },
}
