use anyhow::Result;
use clap::{self as clap, Clap};
use futures::{Stream, StreamExt};
use minidsp::utils::recorder::Message;
use minidsp::{
    commands::Commands,
    packet,
    utils::{decoder, recorder},
};
use std::borrow::BorrowMut;
use std::convert::TryInto;
use std::path::PathBuf;
use tokio::{fs::File, io::AsyncReadExt};
use tokio_util::{
    codec::{Decoder, LinesCodec},
    io::StreamReader,
};

#[derive(Clap, Debug)]
#[clap(version=env!("CARGO_PKG_VERSION"), author=env!("CARGO_PKG_AUTHORS"))]
struct Opts {
    filename: PathBuf,

    #[clap(subcommand)]
    cmd: SubCommand,
}

#[derive(Clap, Debug)]
enum SubCommand {
    /// Pretty-print protocol decodes
    Decode,

    /// Dumps the bulk-loaded parameter data into a file
    DumpBulk {
        output: PathBuf,
        #[clap(long)]
        skip: Option<usize>,
    },
}

#[tokio::main]
pub async fn main() -> Result<()> {
    env_logger::init();
    let opts: Opts = Opts::parse();

    let file = File::open(opts.filename).await?;
    let framed = LinesCodec::new().framed(file);
    let messages =
        framed.filter_map(|x| async { recorder::Message::from_string(x.ok()?.as_str()) });

    match opts.cmd {
        SubCommand::Decode => {
            decode(messages).await?;
        }
        SubCommand::DumpBulk { output, skip } => {
            dump(output, skip, messages).await?;
        }
    }

    Ok(())
}

async fn dump(
    output: PathBuf,
    skip: Option<usize>,
    framed: impl Stream<Item = recorder::Message>,
) -> Result<()> {
    // Only keep bulk load commands
    let f = framed
        .filter_map(|x| async {
            if let Message::Sent(data) = x {
                let data = packet::unframe(data).ok()?;
                Some(Commands::from_bytes(data).ok()?)
            } else {
                None
            }
        })
        .filter_map(|x| async {
            match x {
                Commands::BulkLoad { payload } => Some(Ok(payload.0)),
                Commands::Write { .. } => Some(Err(std::io::ErrorKind::UnexpectedEof)),
                _ => None,
            }
        });

    // let f = Box::pin(f);
    let mut reader = Box::pin(StreamReader::new(f));
    let mut output = File::create(output).await?;

    if let Some(skip) = skip {
        tokio::io::copy(
            &mut reader.borrow_mut().take(skip.try_into().unwrap()),
            &mut tokio::io::sink(),
        )
        .await?;
    }

    tokio::io::copy(&mut reader, &mut output).await?;

    Ok(())
}

async fn decode(framed: impl Stream<Item = recorder::Message>) -> Result<()> {
    let mut decoder = {
        use termcolor::{ColorChoice, StandardStream};
        let writer = StandardStream::stdout(ColorChoice::Always);
        decoder::Decoder {
            w: Box::new(writer),
            quiet: true,
        }
    };

    let mut n_recv: i32 = 0;
    let mut n_sent: i32 = 0;
    let mut framed = Box::pin(framed);

    while let Some(msg) = framed.next().await {
        match msg {
            recorder::Message::Sent(data) => {
                n_sent += 1;
                print!("{}:", n_sent);
                decoder.feed_sent(&data);
            }
            recorder::Message::Received(data) => {
                n_recv += 1;
                print!("{}:", n_recv);
                decoder.feed_recv(&data);
            }
        }
    }

    Ok(())
}
