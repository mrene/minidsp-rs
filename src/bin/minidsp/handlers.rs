use super::{InputCommand, MiniDSP, OutputCommand, Result};
use crate::debug::run_debug;
use crate::{PEQCommand, RoutingCommand, SubCommand};
use arrayvec::ArrayVec;
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
        Some(SubCommand::Cec) => run_cec(&device).await?,
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
            let delay = Duration::from_secs_f32(delay / 1000.);
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

pub(crate) async fn run_cec(dsp: &MiniDSP<'_>) -> Result<(), anyhow::Error> {
    use cec_rs::{CecConnectionCfgBuilder, CecDeviceType, CecDeviceTypeVec, CecConnection, CecCommand, CecOpcode,  CecDatapacket, CecLogicalAddress};

    let cfg = CecConnectionCfgBuilder::default()
        .port("RPI".into())
        .device_name("MiniDSP".into())
        .key_press_callback(Box::new(on_key_press))
        .command_received_callback(Box::new(on_command_received))
        .device_types(CecDeviceTypeVec::new(CecDeviceType::AudioSystem))
        .activate_source(false)
        .build()
        .unwrap();
    let connection: CecConnection = cfg.open().unwrap();
    println!("Active source: {:?}", connection.get_active_source());

    for i in 0..100 {

        let mut parameters = ArrayVec::new();
        parameters.push(i as u8);
    
        let audio_report = CecCommand {
            initiator: CecLogicalAddress::Audiosystem,
            destination: CecLogicalAddress::Tv,
            ack: false,
            eom: true,
            opcode: CecOpcode::GiveAudioStatus,
            parameters: CecDatapacket(parameters.clone()),
            opcode_set: true,
            transmit_timeout: Duration::from_secs(1),
        };
        connection.transmit(audio_report.into()).unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    Ok(())
}

fn on_command_received(command: cec_rs::CecCommand) {
    println!(
        "initiator={:?} destination={:?} op={:?} params={:?}",
        command.initiator, command.destination, command.opcode, command.parameters
    );
}

fn on_key_press(keypress: cec_rs::CecKeypress) {
    println!("{:?}", keypress);
}
