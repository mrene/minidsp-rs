extern crate clap;
extern crate hidapi;
use clap::{App, Arg, ArgMatches, SubCommand};
use minidsp::transport::hid::find_minidsp;
use minidsp::{Gain, MiniDSP, Source};

#[tokio::main]
async fn main() {
    let matches = App::new("minidsp")
        .version("1.0")
        .author("Mathieu Rene")
        .about("Controls a MiniDSP via HID commands")
        .arg(Arg::with_name("verbose").short("v").help("Log HID reports"))
        .arg(
            Arg::with_name("log")
                .short("l")
                .help("logs request-response pairs"),
        )
        .subcommand(
            SubCommand::with_name("gain")
                .help("Sets the gain in decibels [-127, 0]")
                .arg(Arg::with_name("value").required(true).index(1)),
        )
        .subcommand(
            SubCommand::with_name("mute")
                .help("Mutes or unmutes the master output")
                .arg(
                    Arg::with_name("value")
                        .required(false)
                        .index(1)
                        .possible_values(&["on", "off"]),
                ),
        )
        .subcommand(
            SubCommand::with_name("source")
                .help("Changes the active audio source")
                .arg(
                    Arg::with_name("source")
                        .required(true)
                        .index(1)
                        .possible_values(&["analog", "toslink", "usb"]),
                ),
        )
        .subcommand(
            SubCommand::with_name("send")
                .help("Sends a hex packet and compute length + crc")
                .arg(
                    Arg::with_name("data")
                        .required(true)
                        .index(1)
                        .help("Hex-encoded data"),
                ),
        )
        .get_matches();

    let result = run_command(matches).await;
    if let Err(error) = result {
        eprintln!("{:?}", error);
    }
}

async fn run_command(matches: ArgMatches<'_>) -> anyhow::Result<()> {
    let transport = Box::new(find_minidsp()?);
    // if matches.occurrences_of("verbose") > 0 {
    //     transport.verbose = true
    // }
    //
    // if matches.occurrences_of("log") > 0 {
    //     transport.log = true
    // }

    let device = MiniDSP::new(transport);

    if let Some(matches) = matches.subcommand_matches("gain") {
        let value = matches.value_of("value").unwrap();
        let value = i8::from_str_radix(value, 10).unwrap();

        println!("set gain: {:?}", value);
        Ok(device.set_master_volume(Gain(value as f32)).await?)
    } else if let Some(matches) = matches.subcommand_matches("mute") {
        let value = matches.value_of("value").unwrap().to_lowercase();
        let value = match value.as_str() {
            "on" => true,
            "off" => false,
            _ => true,
        };

        println!("mute {:?}", value);
        Ok(device.set_master_mute(value).await?)
    } else if let Some(matches) = matches.subcommand_matches("source") {
        let source = matches.value_of("source").unwrap().to_lowercase();
        let source = match source.as_str() {
            "analog" => Source::Analog,
            "toslink" => Source::Toslink,
            "usb" => Source::Usb,
            _ => panic!("invalid source"),
        };

        println!("set source {:?}", source);
        Ok(device.set_source(source).await?)
    } else if let Some(matches) = matches.subcommand_matches("send") {
        let value = matches.value_of("data").unwrap();
        let _value = hex::decode(value.replace(" ", ""))?;

        // let response = device.transport.roundtrip(value.as_ref())?;
        // println!("response: {:02x?}", response);

        Ok(())
    } else {
        let master_status = device.get_master_status().await?;
        println!("{:?}", master_status);
        Ok(())
    }
}
