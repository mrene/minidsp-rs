use std::{fmt::Debug, sync::Arc, time::Duration};

use bytes::{Buf, Bytes};
use futures::{channel::mpsc, pin_mut, Sink, SinkExt, Stream, StreamExt};
use minidsp_protocol::{
    commands::{BytesWrap, Responses},
    device::probe_kind,
    packet, Commands, DeviceInfo,
};
use strong_xml::XmlRead;
use tokio::sync::Mutex;
use url2::Url2;

use super::Transport;
use crate::{
    formats::xml_config::Setting,
    utils::{mock_device::MockDevice, Combine, OwnedJoinHandle},
    MiniDSPError,
};

pub struct MockTransport {
    pub device: Arc<Mutex<MockDevice>>,
    // Handle to the task handling commands on this device
    #[allow(dead_code)]
    task: OwnedJoinHandle<()>,

    transport: Transport,
}

impl MockTransport {
    pub fn new(hw_id: u8, dsp_version: u8) -> Self {
        let kind = probe_kind(&DeviceInfo {
            dsp_version,
            hw_id,
            serial: 0,
        });
        let device: Arc<Mutex<MockDevice>> =
            Arc::new(Mutex::new(MockDevice::new(hw_id, dsp_version, kind)));
        let (commands_tx, commands_rx) = mpsc::unbounded::<Commands>();
        let (responses_tx, responses_rx) = mpsc::unbounded::<Responses>();
        let task = tokio::spawn(Self::task(device.clone(), commands_rx, responses_tx)).into();

        let commands_tx = commands_tx
            .sink_map_err(|_| MiniDSPError::TransportClosed)
            .with(|cmd| async move {
                let mut unframed = packet::unframe(cmd).unwrap();
                let cmd = Commands::from_bytes(unframed.clone());
                let cmd = match cmd {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("Command decode error: {:?}", e);
                        Commands::Unknown {
                            cmd_id: unframed.get_u8(),
                            payload: BytesWrap(unframed),
                        }
                    }
                };

                Ok::<_, MiniDSPError>(cmd)
            });

        let responses_rx =
            responses_rx.map(|resp| Ok::<_, MiniDSPError>(packet::frame(resp.to_bytes())));

        let transport = Box::pin(Combine::new(responses_rx, commands_tx)) as Transport;

        Self {
            device,
            task,
            transport,
        }
    }

    async fn task(
        device: Arc<Mutex<MockDevice>>,
        commands_rx: impl Stream<Item = Commands>,
        responses_tx: impl Sink<Responses, Error = impl Debug>,
    ) {
        pin_mut!(commands_rx);
        pin_mut!(responses_tx);

        while let Some(command) = commands_rx.next().await {
            let mut device = device.lock().await;
            let response = device.execute(&command);
            if let Some(duration) = device.response_delay {
                tokio::time::sleep(duration).await;
            }
            responses_tx.send(response).await.unwrap();
        }
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new(10, 100)
    }
}

pub fn open_url(url: &Url2) -> Transport {
    let (mut hw_id, mut dsp_version) = (10, 100);
    for (key, value) in url.query_pairs() {
        if key == "hw_id" {
            hw_id = value.parse().unwrap();
        } else if key == "dsp_version" {
            dsp_version = value.parse().unwrap()
        }
    }
    let mock = MockTransport::new(hw_id, dsp_version);

    let mut device = mock.device.try_lock().unwrap();
    for (key, value) in url.query_pairs() {
        if key == "response_delay" {
            let value = value.parse().unwrap();
            device.response_delay = Some(Duration::from_millis(value));
        } else if key == "serial" {
            device.set_serial(value.parse().unwrap());
        } else if key == "timestamp" {
            device.set_timestamp(value.parse().unwrap());
        } else if key == "firmware_version" {
            let parts = value.split('.').collect::<Vec<_>>();
            if parts.len() < 2 {
                panic!("invalid firmware version, use format 1.13")
            }
            let firmware_version = (parts[0].parse().unwrap(), parts[1].parse().unwrap());
            device.firmware_version = firmware_version;
        }
    }
    {
        let cfg = std::fs::read_to_string(
            r"C:\Users\mrene\Documents\MiniDSP\MiniDSP-2x8-nanoDIGI\setting\setting1.xml",
        )
        .unwrap();
        let s = Setting::from_str(&cfg).unwrap();
        println!("GOT TS: {}", s.timestamp);
        device.set_timestamp(s.timestamp);
    }

    drop(device);

    Box::pin(mock)
}

impl Stream for MockTransport {
    type Item = <Transport as Stream>::Item;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.transport.as_mut().poll_next(cx)
    }
}

impl Sink<Bytes> for MockTransport {
    type Error = <Transport as Sink<Bytes>>::Error;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.transport.as_mut().poll_ready(cx)
    }

    fn start_send(mut self: std::pin::Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        self.transport.as_mut().start_send(item)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.transport.as_mut().poll_flush(cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.transport.as_mut().poll_close(cx)
    }
}
