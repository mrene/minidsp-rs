use std::{path::PathBuf, sync::Arc};

use bytes::Bytes;
use futures::{pin_mut, StreamExt};
use tokio::sync::Mutex;

use crate::{
    transport::Transport,
    utils::{self, decoder::Decoder, logger, recorder::Recorder},
};

pub fn transport_logging(
    transport: Transport,
    verbose: u8,
    log: Option<PathBuf>,
) -> (Option<Arc<Mutex<Decoder>>>, Transport) {
    let (log_tx, log_rx) = futures::channel::mpsc::unbounded::<utils::Message<Bytes, Bytes>>();
    let transport = logger(transport, log_tx);

    let decoder = if verbose > 0 {
        use termcolor::{ColorChoice, StandardStream};
        let writer = StandardStream::stderr(ColorChoice::Auto);
        Some(Arc::new(Mutex::new(Decoder::new(
            Box::new(writer),
            verbose == 1,
            None,
        ))))
    } else {
        None
    };
    {
        let decoder = decoder.clone();
        tokio::spawn(async move {
            let result = async move {
                let mut recorder = match log {
                    Some(filename) => Some(Recorder::new(tokio::fs::File::create(filename).await?)),
                    _ => None,
                };

                pin_mut!(log_rx);

                while let Some(msg) = log_rx.next().await {
                    match msg {
                        utils::Message::Sent(msg) => {
                            if let Some(decoder) = &decoder {
                                decoder.lock().await.feed_sent(&msg);
                            }
                            if let Some(recorder) = recorder.as_mut() {
                                recorder.feed_sent(&msg);
                            }
                        }
                        utils::Message::Received(msg) => {
                            if let Some(decoder) = &decoder {
                                decoder.lock().await.feed_recv(&msg);
                            }
                            if let Some(recorder) = recorder.as_mut() {
                                recorder.feed_recv(&msg);
                            }
                        }
                    }
                }

                Ok::<(), anyhow::Error>(())
            };

            if let Err(e) = result.await {
                log::error!("transport logging exiting: {}", e);
            }
        });
    }

    (decoder, Box::pin(transport))
}
