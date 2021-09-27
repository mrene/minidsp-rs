use std::sync::Arc;

use bytes::Bytes;
use futures::{
    channel::mpsc::{self, UnboundedReceiver, UnboundedSender},
    pin_mut, Future, FutureExt, SinkExt, StreamExt,
};
use minidsp::{
    client::Client,
    transport::{Multiplexer, Transport},
    utils::Combine,
    Channel, DeviceInfo, MiniDSP, MiniDSPError,
};
use tokio::sync::Mutex;
use hex_literal::hex;

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
        let dsp = MiniDSP::from_client(
            client,
            &minidsp_protocol::device::m2x4hd::DEVICE,
            DeviceInfo {
                hw_id,
                dsp_version,
                serial: 0,
            },
        );

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
                    println!("{} vs {}", hex::encode(&cmd), hex::encode(expect_cmd.as_ref()));
                    assert_eq!(&cmd, expect_cmd.as_ref());
                    self.responses_tx.send(Ok(Bytes::from_static(response))).await.unwrap();
                }
            }
        }
    }
}

#[tokio::test]
async fn test_2x4hd() -> anyhow::Result<()> {
    let (mut dev, dsp) = TestDevice::new(10, 100);

    let input = dsp.input(0)?;
    {
        // Gain & Mute
        dev.run(
            input.set_mute(true),
            hex!("09 13 800000 01000000 9d"),
            &[0x1],
        )
        .await
        .unwrap();
        dev.run(
            input.set_mute(false),
            hex!("09 13 800000 02000000 9e"),
            &[0x1],
        )
        .await
        .unwrap();

    }
    {
        // Input PEQs
        let peq = input.peq(0)?;
        dev.run(
            peq.set_bypass(true),
            hex!("05 19 802085 43"),
            &[0x01],
        )
        .await
        .unwrap();
        dev.run(
            peq.set_bypass(false),
            hex!("05 19 002085 c3"),
            &[0x01],
        )
        .await
        .unwrap();

        dev.run(
            peq.set_coefficients(&[1.0, 0.2, 0.3, 0.4, 0.5]),
            hex!("1b 30 802085 0000 0000803f cdcc4c3e 9a99993e cdcccc3e 0000003f 3e"),
            &[0x01],
        )
        .await
        .unwrap();
    }

    Ok(())
}
