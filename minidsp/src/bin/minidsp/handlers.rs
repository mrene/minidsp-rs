use std::{str::FromStr, time::Duration};

use minidsp::{
    formats::{rew::FromRew, wav::read_wav_filter},
    model::StatusSummary,
    transport::Transport,
    Biquad, BiquadFilter, Channel, Crossover, DeviceInfo, Fir, MiniDSPError,
};

use super::{InputCommand, MiniDSP, OutputCommand, Result};
use crate::{debug::run_debug, FilterCommand, PEQTarget, RoutingCommand, SubCommand, ToggleBool};

pub(crate) async fn run_server(
    subcmd: SubCommand,
    transport: Transport,
    device_info: DeviceInfo,
) -> Result<()> {
    let transport = Box::pin(transport);

    if let SubCommand::Server {
        bind_address,
        advertise,
        ip,
    } = subcmd
    {
        if let Some(hostname) = advertise {
            use std::net::Ipv4Addr;

            use crate::discovery;
            let mut packet = discovery::DiscoveryPacket {
                // The mac address is used to distinguish between devices on the *device list* page in the MiniDSP Device Console app
                mac_address: [0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00],
                ip_address: Ipv4Addr::UNSPECIFIED,
                hwid: device_info.hw_id,
                dsp_id: device_info.dsp_version,
                fw_major: device_info.fw_major,
                fw_minor: device_info.fw_minor,
                sn: ((device_info.serial - 900000) & 0xFFFF) as u16,
                hostname,
            };
            if let Some(ip) = ip {
                packet.ip_address = Ipv4Addr::from_str(&ip)?;
                packet.mac_address[2..].copy_from_slice(&packet.ip_address.octets());
            } else {
                return Err(anyhow::anyhow!("--ip is required if --advertise is used"));
            }
            let interval = Duration::from_secs(1);
            tokio::spawn(discovery::server::advertise_packet(
                None,
                move || Some(packet.clone()),
                interval,
            ));
        }
        use crate::tcp_server;
        tcp_server::serve(bind_address.as_str(), transport).await?;
    }
    Ok(())
}

pub(crate) async fn run_command(
    device: &MiniDSP<'_>,
    cmd: Option<&SubCommand>,
    opts: &crate::Opts,
) -> Result<()> {
    match cmd {
        // Master
        Some(&SubCommand::Gain {
            mut value,
            relative,
        }) => {
            if relative {
                let status = device.get_master_status().await?;
                value.0 += status.volume.unwrap().0;
            }
            device.set_master_volume(value).await?
        }
        Some(&SubCommand::Mute { value }) => {
            let mute: bool = match value {
                ToggleBool::On => true,
                ToggleBool::Off => false,
                ToggleBool::Toggle => !device.get_master_status().await?.mute.unwrap(),
            };
            device.set_master_mute(mute).await?
        }
        Some(&SubCommand::Source { value }) => device.set_source(value).await?,
        Some(&SubCommand::Config { value }) => device.set_config(value).await?,
        Some(&SubCommand::Dirac { value }) => {
            let dirac: bool = match value {
                ToggleBool::On => true,
                ToggleBool::Off => false,
                ToggleBool::Toggle => !device.get_master_status().await?.dirac.unwrap(),
            };
            device.set_dirac(dirac).await?
        }
        Some(&SubCommand::Input {
            input_index,
            ref cmd,
        }) => run_input(device, cmd, input_index).await?,
        Some(&SubCommand::Output {
            output_index,
            ref cmd,
        }) => run_output(device, output_index, cmd).await?,

        // Other tools
        Some(&SubCommand::Debug { ref cmd }) => run_debug(device, cmd).await?,

        // Handled earlier
        Some(&SubCommand::Server { .. }) => {}
        Some(&SubCommand::Probe { .. }) => return Ok(()),

        Some(&SubCommand::Status) | None => {
            // Always output the current master status and input/output levels
            let summary = StatusSummary::fetch(device).await?;
            println!("{}", opts.output_format.format(&summary));
        }
    };

    Ok(())
}

pub(crate) async fn run_input(
    dsp: &MiniDSP<'_>,
    cmd: &InputCommand,
    input_index: usize,
) -> Result<()> {
    use InputCommand::*;
    use RoutingCommand::*;

    let input = dsp.input(input_index)?;
    match *cmd {
        InputCommand::Gain { value } => input.set_gain(value).await?,
        Mute { value } => input.set_mute(value.0).await?,
        Routing {
            output_index,
            ref cmd,
        } => match *cmd {
            Enable { value } => input.set_output_enable(output_index, value.0).await?,
            RoutingCommand::Gain { value } => input.set_output_gain(output_index, value).await?,
        },
        PEQ { index, ref cmd } => match index {
            PEQTarget::One(index) => run_peq(&[input.peq(index)?], cmd).await?,
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
    cmd: &OutputCommand,
) -> Result<()> {
    use OutputCommand::*;
    let output = dsp.output(output_index)?;

    match cmd {
        &OutputCommand::Gain { value } => output.set_gain(value).await?,
        &Mute { value } => output.set_mute(value.0).await?,
        &Delay { delay } => {
            let delay = Duration::from_secs_f32(delay / 1000.);
            output.set_delay(delay).await?
        }
        &Invert { value } => output.set_invert(value.0).await?,
        &PEQ { index, ref cmd } => match index {
            PEQTarget::One(index) => run_peq(&[output.peq(index)?], cmd).await?,
            PEQTarget::All => {
                let eqs = output.peqs_all();
                run_peq(eqs.as_ref(), cmd).await?
            }
        },
        FIR { cmd } => {
            run_fir(
                dsp,
                &output.fir().ok_or(MiniDSPError::NoSuchPeripheral)?,
                cmd,
            )
            .await?
        }
        &Crossover {
            group,
            index,
            ref cmd,
        } => {
            run_xover(
                &output.crossover().ok_or(MiniDSPError::NoSuchPeripheral)?,
                cmd,
                group,
                index,
            )
            .await?
        }
        &Compressor {
            bypass,
            threshold,
            ratio,
            attack,
            release,
        } => {
            let compressor = output.compressor().ok_or(MiniDSPError::NoSuchPeripheral)?;
            if let Some(bypass) = bypass {
                compressor.set_bypass(bypass.0).await?;
            }
            if let Some(threshold) = threshold {
                compressor.set_threshold(threshold).await?;
            }
            if let Some(ratio) = ratio {
                compressor.set_ratio(ratio).await?;
            }
            if let Some(attack) = attack {
                compressor.set_attack(attack).await?;
            }
            if let Some(release) = release {
                compressor.set_release(release).await?;
            }
        }
    }
    Ok(())
}

pub(crate) async fn run_peq(peqs: &[BiquadFilter<'_>], cmd: &FilterCommand) -> Result<()> {
    use FilterCommand::*;

    match cmd {
        Set { coeff } => {
            if peqs.len() > 1 {
                eprintln!("Warning: Setting the same coefficients on all PEQs, did you mean `peq [n] set` instead?")
            }
            for peq in peqs {
                peq.set_coefficients(coeff).await?;
            }
        }
        &Bypass { value } => {
            for peq in peqs {
                peq.set_bypass(value.0).await?;
            }
        }
        Clear => {
            for peq in peqs {
                peq.clear().await?;
                peq.set_bypass(false).await?;
            }
        }
        FilterCommand::Import { filename, .. } => {
            let file = std::fs::read_to_string(filename)?;
            let mut lines = file.lines();
            for (i, peq) in peqs.iter().enumerate() {
                if let Some(biquad) = Biquad::from_rew_lines(&mut lines) {
                    peq.set_coefficients(biquad.to_array().as_ref()).await?;
                    println!(
                        "PEQ {}: Applied imported filter: biquad{}",
                        i,
                        biquad.index.unwrap_or_default()
                    );
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

pub(crate) async fn run_xover(
    xover: &Crossover<'_>,
    cmd: &FilterCommand,
    group: usize,
    index: PEQTarget,
) -> Result<()> {
    match cmd {
        FilterCommand::Set { coeff } => match index {
            PEQTarget::All => {
                for index in 0..xover.num_filter_per_group() {
                    xover.set_coefficients(group, index, coeff.as_ref()).await?;
                }
            }
            PEQTarget::One(index) => {
                xover.set_coefficients(group, index, coeff.as_ref()).await?;
            }
        },
        &FilterCommand::Bypass { value } => {
            xover.set_bypass(group, value.0).await?;
        }
        &FilterCommand::Clear => {
            xover.clear(group).await?;
        }
        FilterCommand::Import { filename, .. } => {
            let file = std::fs::read_to_string(filename)?;
            let mut lines = file.lines();

            let range = match index {
                PEQTarget::All => 0..xover.num_filter_per_group(),
                PEQTarget::One(i) => i..i + 1,
            };

            for i in range {
                if let Some(biquad) = Biquad::from_rew_lines(&mut lines) {
                    xover
                        .set_coefficients(group, i, biquad.to_array().as_ref())
                        .await?;
                    println!(
                        "Xover {}.{}: Applied imported filter: biquad{}",
                        group,
                        i,
                        biquad.index.unwrap_or_default()
                    );
                } else {
                    println!("Xover {}.{}: Cleared filter", group, i);
                    xover.clear(group).await?;
                }
            }

            xover.set_bypass(group, false).await?;

            if Biquad::from_rew_lines(&mut lines).is_some() {
                eprintln!("Warning: Some filters were not imported because they didn't fit (try using `all`)")
            }
        }
    }

    Ok(())
}

pub(crate) async fn run_fir(dsp: &MiniDSP<'_>, fir: &Fir<'_>, cmd: &FilterCommand) -> Result<()> {
    match cmd {
        FilterCommand::Set { coeff } => {
            fir.set_coefficients(coeff.as_ref()).await?;
        }
        &FilterCommand::Bypass { value } => {
            fir.set_bypass(value.0).await?;
        }
        &FilterCommand::Clear => {
            fir.clear().await?;
        }
        FilterCommand::Import { filename, .. } => {
            let coeff = read_wav_filter(filename, dsp.device.internal_sampling_rate)?;
            fir.set_coefficients(coeff.as_ref()).await?;
        }
    }

    Ok(())
}
