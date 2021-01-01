use super::{InputCommand, MiniDSP, OutputCommand, Result};
use crate::debug::run_debug;
use crate::{PEQCommand, RoutingCommand, SubCommand};
use minidsp::{BiquadFilter, Channel};
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
            server::serve(bind_address, device.transport.clone()).await?
        }
        // Handled earlier
        Some(SubCommand::Probe) => return Ok(()),
        Some(SubCommand::Debug(debug)) => run_debug(&device, debug).await?,
        None => {}
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
        PEQ { index, cmd } => run_peq(input.peq(index), cmd).await?,
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
            let delay = Duration::from_secs_f32(delay * 1000.);
            output.set_delay(delay).await?
        }
        Invert { value } => output.set_invert(value).await?,
        PEQ { index, cmd } => run_peq(output.peq(index), cmd).await?,
    }
    Ok(())
}

pub(crate) async fn run_peq(peq: BiquadFilter<'_>, cmd: PEQCommand) -> Result<()> {
    use PEQCommand::*;

    match cmd {
        Set { coeff } => {
            peq.set_coefficients(&coeff).await?;
        }
        Bypass { value } => peq.set_bypass(value).await?,
    }
    Ok(())
}
