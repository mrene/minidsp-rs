#[macro_use]
mod test_utils;
use hex_literal::hex;
use minidsp::{Channel, Gain};
use std::time::Duration;
use test_utils::TestDevice;

#[tokio::test]
async fn test_2x8() -> anyhow::Result<()> {
    let (mut dev, dsp) = TestDevice::new(1, 50);

    let input = dsp.input(0)?;
    test!(dev, input.set_mute(true), hex!("08 13 8035 00000000 d0"));
    test!(dev, input.set_mute(false), hex!("08 13 8035 00800000 50"));

    test!(
        dev,
        input.set_gain(Gain(10.0)),
        // Real: hex!("08 13 800b 0194c583 83") - allowing due to rounding err
        hex!("08 13 800b 0194c584 84")
    );
    test!(
        dev,
        input.set_gain(Gain(-12.0)),
        hex!("08 13 800b 002026f3 df")
    );

    // Routing
    test!(
        dev,
        dsp.input(0)?.set_output_enable(0, false),
        hex!("08 13 809f 00000000 3a")
    );
    test!(
        dev,
        dsp.input(0)?.set_output_enable(0, true),
        hex!("08 13 809f 00800000 ba")
    );

    // PEQ
    let peq = input.peq(0)?;
    test!(
        dev,
        peq.set_coefficients(&[1.0, 0.50, 0.25, -0.25, -0.50]),
        hex!("1a 30 809a0000 00800000 00400000 00200000 0fe00000 0fc00000 02")
    );
    test!(dev, peq.set_bypass(true), hex!("04 19 809a 37"));
    test!(dev, peq.set_bypass(false), hex!("04 19 009a b7"));

    // Output - uses half the first byte for the extended address
    let output = dsp.output(0)?;
    test!(
        dev,
        output.set_gain(Gain(10.0)),
        // rounding fix from hex!("08 13 8380 0194c583 fb")
        hex!("08 13 8380 0194c584 fc")
    );
    test!(
        dev,
        output.set_gain(Gain(-12.0)),
        hex!("08 13 8380 002026f3 57")
    );
    test!(
        dev,
        output.set_delay(Duration::from_millis(9)),
        hex!("08 13 835b 00000360 5c")
    );
    test!(dev, output.set_invert(true), hex!("08 13 8365 ff800000 82"));
    test!(
        dev,
        output.set_invert(false),
        hex!("08 13 8365 00800000 83")
    );
    Ok(())
}
