use super::{InputCommand, MiniDSP, OutputCommand, Result};
use crate::{PEQCommand, RoutingCommand};
use minidsp::{BiquadFilter, Channel};
use std::time::Duration;

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
