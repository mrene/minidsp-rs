#[cfg(target_family = "unix")]
use std::path::{Path, PathBuf};
use std::{future, str::FromStr};

#[cfg(target_family = "unix")]
use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use hyper::Uri;
use tokio::io::{AsyncRead, AsyncWrite};
#[cfg(target_family = "unix")]
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{
    tungstenite::{self, Message},
    WebSocketStream,
};
use url2::{Url2, Url2Error};

#[cfg(target_family = "unix")]
use super::Openable;
use super::Transport;
use crate::MiniDSPError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("URL Error: {0}")]
    UrlError(#[from] url2::Url2Error),

    // #[error("Invalid Uri: {0}")]
    // InvalidUri(#[from] <Uri as FromStr>::Err),
    #[error("WebSocket error: {0}")]
    WsError(#[from] tungstenite::Error),

    #[error("Hyper error: {0}")]
    HyperError(#[from] hyper::Error),

    #[error("Deserialization error: {0}")]
    DeserializeError(#[from] serde_json::Error),
}

pub async fn open_url(url: &Url2) -> Result<Transport, Error> {
    let (stream, _) = tokio_tungstenite::connect_async(&**url).await?;
    Ok(transport_from_websocket_stream(stream))
}

#[cfg(target_family = "unix")]
pub async fn open_unix(socket_path: &Path, url: &str) -> Result<Transport, Error> {
    let request = url.into_client_request()?;

    let sock = tokio::net::UnixStream::connect(socket_path)
        .await
        .map_err(tungstenite::Error::Io)?;

    let (stream, _) = tokio_tungstenite::client_async(request, sock).await?;
    Ok(transport_from_websocket_stream(stream))
}

/// Converts a WebSocket stream into a transport by mapping it to binary messages
fn transport_from_websocket_stream<T>(ws: WebSocketStream<T>) -> Transport
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let stream = ws
        .map(|item| -> Result<Bytes, MiniDSPError> {
            log::trace!("websocket rx: {:?}", &item);
            match item {
                Ok(msg) => Ok(Bytes::from(msg.into_data())),
                Err(e) => Err(MiniDSPError::TransportFailure(e.to_string())),
            }
        })
        .with(|item: Bytes| {
            log::trace!("websocket tx: {:?}", &item);
            future::ready(Ok(Message::from(item.to_vec())))
        })
        .sink_map_err(|e: tungstenite::Error| MiniDSPError::TransportFailure(e.to_string()));

    Box::pin(stream)
}

#[derive(Clone, Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Device {
    url: String,
    version: Option<crate::DeviceInfo>,
    product_name: Option<String>,
}

/// Generates a list of URLs that can be opened with [`open_url`] from all devices managed by the given server
pub async fn discover(base_url: &Url2) -> Result<Vec<Url2>, Error> {
    use hyper::Client;

    let client = Client::new();
    let mut resp = client
        .get(
            Uri::from_str(
                base_url
                    .join("/devices")
                    .map_err(Url2Error::from)?
                    .to_string()
                    .as_str(),
            )
            .expect("parsing generated url failed"),
        )
        .await?;

    let bytes = hyper::body::to_bytes(resp.body_mut()).await?;
    let devices: Vec<Device> = serde_json::from_slice(&bytes)?;
    let devices = devices
        .into_iter()
        .enumerate()
        .map(|(i, _)| {
            let mut u = base_url
                .join(&format!("/devices/{i}/ws"))
                .expect("parsing generated url failed (2)");
            u.set_scheme("ws").unwrap();
            Url2::from(u)
        })
        .collect();

    Ok(devices)
}
#[cfg(target_family = "unix")]
pub struct UnixDevice {
    path: PathBuf,
    index: usize,
}

#[cfg(target_family = "unix")]
#[async_trait]
impl Openable for UnixDevice {
    async fn open(&self) -> anyhow::Result<Transport, MiniDSPError> {
        let uri = self.to_url();
        Ok(open_unix(&self.path, &uri).await?)
    }

    fn to_url(&self) -> String {
        format!("ws://localhost/devices/{0}/ws", self.index)
    }
}

#[cfg(target_family = "unix")]
pub async fn discover_unix(path: impl AsRef<Path>) -> Result<Vec<UnixDevice>, Error> {
    use hyper::Client;
    use hyperlocal::{UnixClientExt, Uri};

    let path = path.as_ref();
    let url = Uri::new(path, "/devices").into();
    let client = Client::unix();
    let mut resp = client.get(url).await?;

    let bytes = hyper::body::to_bytes(resp.body_mut()).await?;
    let devices: Vec<Device> = serde_json::from_slice(&bytes)?;
    let devices = devices
        .into_iter()
        .enumerate()
        .map(|(i, _)| UnixDevice {
            path: path.to_owned(),
            index: i,
        })
        .collect();

    Ok(devices)
}
