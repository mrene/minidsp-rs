use super::{InputCommand, MiniDSP, OutputCommand, Result};
use crate::{debug::run_debug, PEQTarget};
use crate::{PEQCommand, RoutingCommand, SubCommand};
use minidsp::{rew::FromRew, Biquad, BiquadFilter, Channel};
use std::str::FromStr;
use std::time::Duration;

pub(crate) async fn run_command(device: &MiniDSP<'_>, cmd: Option<SubCommand>) -> Result<()> {
    match cmd {
        // Master
        Some(SubCommand::Gain { value }) => device.set_master_volume(value).await?,
        Some(SubCommand::Mute { value }) => device.set_master_mute(value).await?,
        Some(SubCommand::Source { value }) => device.set_source(&value).await?,
        Some(SubCommand::Config { value }) => device.set_config(value).await?,
        Some(SubCommand::Input { input_index, cmd }) => {
            run_input(&device, cmd, input_index).await?
        }
        Some(SubCommand::Output { output_index, cmd }) => {
            run_output(&device, output_index, cmd).await?
        }

        // Other tools
        Some(SubCommand::Server {
            bind_address,
            advertise,
            ip,
        }) => {
            if let Some(hostname) = advertise {
                use crate::discovery;
                use std::net::Ipv4Addr;
                let mut packet = discovery::DiscoveryPacket {
                    mac_address: [10, 20, 30, 40, 50, 60],
                    ip_address: Ipv4Addr::UNSPECIFIED,
                    hwid: 0,
                    typ: 0,
                    sn: 0,
                    hostname,
                };
                if let Some(ip) = ip {
                    packet.ip_address = Ipv4Addr::from_str(ip.as_str())?;
                }
                let interval = Duration::from_secs(1);
                tokio::spawn(discovery::server::advertise_packet(packet, interval));
            }
            use crate::server;
            server::serve(bind_address.as_str(), device.transport.clone()).await?
        }
        // Handled earlier
        Some(SubCommand::Probe) => return Ok(()),
        Some(SubCommand::Debug(debug)) => run_debug(&device, debug).await?,
        None => {
            // Always output the current master status and input/output levels
            let master_status = device.get_master_status().await?;
            println!("{:?}", master_status);

            let input_levels = device.get_input_levels().await?;
            let strs: Vec<String> = input_levels.iter().map(|x| format!("{:.1}", *x)).collect();
            println!("Input levels: {}", strs.join(", "));

            let output_levels = device.get_output_levels().await?;
            let strs: Vec<String> = output_levels.iter().map(|x| format!("{:.1}", *x)).collect();
            println!("Output levels: {}", strs.join(", "));
        }
    };

    Ok(())
}

pub(crate) async fn run_input(
    dsp: &MiniDSP<'_>,
    cmd: InputCommand,
    input_index: usize,
) -> Result<()> {
    use InputCommand::*;
    use RoutingCommand::*;

    let input = dsp.input(input_index);
    match cmd {
        InputCommand::Gain { value } => input.set_gain(value).await?,
        Mute { value } => input.set_mute(value).await?,
        Routing { output_index, cmd } => match cmd {
            Enable { value } => input.set_output_enable(output_index, value).await?,
            RoutingCommand::Gain { value } => input.set_output_gain(output_index, value).await?,
        },
        PEQ { index, cmd } => match index {
            PEQTarget::One(index) => run_peq(&[input.peq(index)], cmd).await?,
            PEQTarget::All => {
                let eqs = input.peqs_all();
                run_peq(eqs.as_ref(), cmd).await?
            }
        },
    }
    Ok(())
}

pub(crate) async fn run_output(
    dsp: &MiniDSP<'_>,
    output_index: usize,
    cmd: OutputCommand,
) -> Result<()> {
    use OutputCommand::*;
    let output = dsp.output(output_index);

    match cmd {
        OutputCommand::Gain { value } => output.set_gain(value).await?,
        Mute { value } => output.set_mute(value).await?,
        Delay { delay } => {
            let delay = Duration::from_secs_f32(delay / 1000.);
            output.set_delay(delay).await?
        }
        Invert { value } => output.set_invert(value).await?,
        OutputCommand::PEQ { index, cmd } => match index {
            PEQTarget::One(index) => run_peq(&[output.peq(index)], cmd).await?,
            PEQTarget::All => {
                let eqs = output.peqs_all();
                run_peq(eqs.as_ref(), cmd).await?
            }
        },
    }
    Ok(())
}

pub(crate) async fn run_peq(peqs: &[BiquadFilter<'_>], cmd: PEQCommand) -> Result<()> {
    use PEQCommand::*;

    match cmd {
        Set { coeff } => {
            if peqs.len() > 1 {
                eprintln!("Warning: Setting the same coefficients on all PEQs, did you mean `peq [n] set` instead?")
            }
            for peq in peqs {
                peq.set_coefficients(&coeff).await?;
            }
        }
        Bypass { value } => {
            for peq in peqs {
                peq.set_bypass(value).await?;
            }
        }
        Clear => {
            for peq in peqs {
                peq.clear().await?;
                peq.set_bypass(false).await?;
            }
        }
        Import { filename } => {
            let file = std::fs::read_to_string(filename)?;
            let mut lines = file.lines();
            for (i, peq) in peqs.iter().enumerate() {
                if let Some(biquad) = Biquad::from_rew_lines(&mut lines) {
                    peq.set_coefficients(biquad.to_array().as_ref()).await?;
                    println!("PEQ {}: Applied imported filter: biquad{}", i, biquad.index);
                } else {
                    println!("PEQ {}: Cleared filter", i);
                    peq.clear().await?;
                }
                peq.set_bypass(false).await?;
            }

            if Biquad::from_rew_lines(&mut lines).is_some() {
                eprintln!("Warning: Some filters were not imported because they didn't fit (try using `all`)")
            }
        }
    }
    Ok(())
}
