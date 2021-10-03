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
        hex!("08 13 800b 0194c583 83")
    );
    test!(
        dev,
        input.set_gain(Gain(-12.0)),
        hex!("08 13 800b 002026f3 df")
    );

    // PEQ
    // Output - uses half the first byte for the extended address
    let output = dsp.output(0)?;
    test!(
        dev,
        output.set_gain(Gain(10.0)),
        hex!("08 13 8380 0194c583 fb")
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

    Ok(())
}
