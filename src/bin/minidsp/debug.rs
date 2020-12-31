use super::DebugCommands;
use anyhow::Result;
use minidsp::commands::{roundtrip, CustomUnaryCommand, ReadFloats, ReadMemory};
use minidsp::MiniDSP;

pub(crate) async fn run_debug(device: &MiniDSP<'_>, debug: DebugCommands) -> Result<()> {
    match debug {
        DebugCommands::Send { value, watch } => {
            let response =
                roundtrip(device.transport.as_ref(), CustomUnaryCommand::new(value)).await?;
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
            let view = roundtrip(device.transport.as_ref(), ReadMemory { addr, size: 60 }).await?;

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
