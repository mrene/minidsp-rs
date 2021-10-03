// #![macro_use]
use std::sync::Arc;

use bytes::Bytes;
use futures::{
    channel::mpsc::{self, UnboundedReceiver, UnboundedSender},
    pin_mut, Future, FutureExt, SinkExt, StreamExt,
};
use minidsp::{
    client::Client,
    device::probe,
    transport::{Multiplexer, Transport},
    utils::Combine,
    DeviceInfo, MiniDSP, MiniDSPError,
};
use tokio::sync::Mutex;

#[allow(unused_macros)]
macro_rules! test {
    ($dev:expr, $cmd:expr, $expect:expr) => {
        $dev.run($cmd, $expect, &[0x1], || stringify!($cmd))
            .await
            .unwrap();
    };
}

pub struct TestDevice {
    commands_rx: UnboundedReceiver<Bytes>,
    responses_tx: UnboundedSender<Result<Bytes, MiniDSPError>>,
}

impl TestDevice {
    pub fn make_transport() -> (
        Transport,
        mpsc::UnboundedReceiver<Bytes>,
        mpsc::UnboundedSender<Result<Bytes, MiniDSPError>>,
    ) {
        let (commands_tx, commands_rx) = mpsc::unbounded::<Bytes>();
        let (responses_tx, responses_rx) = mpsc::unbounded::<Result<Bytes, MiniDSPError>>();

        let commands_tx = commands_tx.sink_map_err(|_| MiniDSPError::TransportClosed);
        let transport = Box::pin(Combine::new(responses_rx, commands_tx)) as Transport;

        (transport, commands_rx, responses_tx)
    }

    pub fn new(hw_id: u8, dsp_version: u8) -> (Self, MiniDSP<'static>) {
        let (transport, commands_rx, responses_tx) = Self::make_transport();
        let mplex = Multiplexer::from_transport(transport);
        let client = Client::new(Arc::new(Mutex::new(mplex.to_service())));
        let device_info = DeviceInfo {
            hw_id,
            dsp_version,
            serial: 0,
        };
        let device = probe(&device_info);
        let dsp = MiniDSP::from_client(client, device, device_info);

        (
            Self {
                commands_rx,
                responses_tx,
            },
            dsp,
        )
    }

    pub async fn run<T>(
        &mut self,
        fut: impl Future<Output = T>,
        expect_cmd: impl AsRef<[u8]>,
        response: &'static [u8],
        msg: impl Fn() -> &'static str,
    ) -> T {
        let cmd = self.commands_rx.next().fuse();
        let fut = fut.fuse();

        pin_mut!(fut);
        pin_mut!(cmd);

        loop {
            futures::select! {
                ret = &mut fut => {
                    return ret;
                },
                cmd = &mut cmd => {
                    let cmd = cmd.unwrap();
                    println!("{}\nactual:\t\t{} \nexpected:\t{}\n", msg(), hex::encode(&cmd), hex::encode(expect_cmd.as_ref()));
                    assert_eq!(&cmd, expect_cmd.as_ref());
                    self.responses_tx.send(Ok(Bytes::from_static(response))).await.unwrap();
                }
            }
        }
    }
}
