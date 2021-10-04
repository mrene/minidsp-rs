#[macro_use]
mod test_utils;
use test_utils::TestDevice;

use hex_literal::hex;
use minidsp::Channel;
use std::time::Duration;

#[tokio::test]
async fn test_2x4hd() -> anyhow::Result<()> {
    let (mut dev, dsp) = TestDevice::new(10, 100);

    let input = dsp.input(0)?;
    {
        // Gain & Mute
        test!(dev, input.set_mute(true), hex!("09 13 800000 01000000 9d"));
        test!(dev, input.set_mute(false), hex!("09 13 800000 02000000 9e"));

        // Input PEQs
        let peq = input.peq(0)?;
        test!(dev, peq.set_bypass(true), hex!("05 19 802085 43"));
    }


    // Routing
    test!(dev, dsp.input(0)?.set_output_enable(0, false), hex!("09 13 800006 01000000 a3"));
    test!(dev, dsp.input(0)?.set_output_enable(0, true), hex!("09 13 800006 02000000 a4"));

    let output = dsp.output(0)?;
    {
        // Gain & Mute
        test!(dev, output.set_mute(true), hex!("09 13 800002 01000000 9f"));
        test!(
            dev,
            output.set_mute(false),
            hex!("09 13 800002 02000000 a0")
        );
    }
    {
        // Delays
        test!(
            dev,
            output.set_delay(Duration::from_micros(10)),
            hex!("09 13 800040 01000000 dd")
        );
        test!(
            dev,
            output.set_delay(Duration::from_millis(1)),
            hex!("09 13 800040 60000000 3c")
        );
    }
    {
        // PEQs
        let peq = output.peq(0)?;
        test!(
            dev,
            peq.clear(),
            hex!("1b 30 8020e9 0000 0000803f 00000000 00000000 00000000 00000000 93")
        );

        test!(
            dev,
            peq.set_coefficients(&[1.0, 0.5, 0.25, -0.25, -0.5]),
            hex!("1b 30 8020e9 0000 0000803f 0000003f 0000803e 000080be 000000bf 8d")
        );
    }

    Ok(())
}
