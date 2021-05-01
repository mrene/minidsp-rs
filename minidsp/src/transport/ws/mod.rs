use std::{future, str::FromStr};

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{tungstenite, tungstenite::Message};
use url2::Url2;

use crate::MiniDSPError;

use super::Transport;

pub async fn open_url(url: &Url2) -> Result<Transport, MiniDSPError> {
    let (stream, _) = tokio_tungstenite::connect_async(&**url).await.unwrap();

    let stream = stream
        .map(|item| -> Result<Bytes, MiniDSPError> {
            match item {
                Ok(msg) => Ok(Bytes::from(msg.into_data())),
                Err(e) => Err(MiniDSPError::TransportFailure(e.to_string())),
            }
        })
        .with(|item: Bytes| future::ready(Ok(Message::from(item.to_vec()))))
        .sink_map_err(|e: tungstenite::Error| MiniDSPError::TransportFailure(e.to_string()));

    Ok(Box::pin(stream))
}

#[derive(Clone, Debug, serde::Deserialize)]
struct Device {
    url: String,
    version: Option<crate::DeviceInfo>,
    product_name: Option<String>,
}
pub async fn discover(base_url: &Url2) -> Result<Vec<(String, crate::DeviceInfo)>, MiniDSPError> {
    use hyper::{Client, Uri};
    // use hyperlocal::UnixClientExt;
    // use std::str::FromStr;

    let client = Client::new();
    let mut resp = client
        .get(Uri::from_str(&base_url.join("/devices").unwrap().to_string()).unwrap())
        .await
        .unwrap();

    let bytes = hyper::body::to_bytes(resp.body_mut()).await.unwrap();
    let devices: Vec<Device> = serde_json::from_slice(&bytes).unwrap();

    let devices = devices
        .into_iter()
        .enumerate()
        .map(|(i, d)| {
            let mut u = base_url.join(&format!("/devices/{}/ws", i)).unwrap();
            u.set_scheme("ws").unwrap();
            (u.to_string(), d.version.unwrap())
        })
        .collect();

    Ok(devices)
}
