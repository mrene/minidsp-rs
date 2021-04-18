use anyhow::Context;
use bytes::Bytes;
use futures::StreamExt;
use hyper_tungstenite::HyperWebsocket;
use minidsp::{transport::Hub, MiniDSPError};
use tungstenite::Message;

/// Bridges frames between a websocket connection and a transport hub
/// The connection is closed as soon as a transport error occurs
pub async fn websocket_transport_bridge(ws: HyperWebsocket, hub: Hub) -> Result<(), anyhow::Error> {
    let websocket = ws.await.context("ws await failed")?;
    let (hub_tx, hub_rx) = hub.split();
    let (ws_tx, ws_rx) = websocket.split();

    let hub_fwd = hub_rx
        .map(|msg| match msg {
            Ok(data) => Ok(Message::Binary(data.to_vec())),
            Err(_) => Err(tungstenite::Error::ConnectionClosed),
        })
        .forward(ws_tx);

    let ws_fwd = ws_rx
        .map(|msg| match msg {
            Ok(Message::Binary(msg)) => Ok(Bytes::from(msg)),
            Ok(_) | Err(_) => Err(MiniDSPError::TransportClosed),
        })
        .forward(hub_tx);

    let (hub_fwd, ws_fwd) = tokio::join!(hub_fwd, ws_fwd);
    hub_fwd?;
    ws_fwd?;

    Ok(())
}
