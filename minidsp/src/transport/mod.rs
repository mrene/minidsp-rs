//! Transport base traits for talking to devices

//! Wraps a Stream + Sink backend into a transport
use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use minidsp_protocol::commands::ProtocolError;
use thiserror::Error;
use tokio::sync::{broadcast, Mutex};
use url2::Url2;

pub type SharedService = Arc<Mutex<MultiplexerService>>;

pub type Transport =
    Pin<Box<dyn StreamSink<'static, Result<Bytes, MiniDSPError>, Bytes, MiniDSPError> + Send>>;

#[cfg(feature = "hid")]
pub mod hid;

#[cfg(feature = "hid")]
use hidapi::HidError;

use crate::utils::StreamSink;

pub mod frame_codec;
pub mod multiplexer;
pub use multiplexer::Multiplexer;
pub mod hub;
pub use hub::Hub;

use self::multiplexer::MultiplexerService;
pub mod net;
pub mod ws;

#[derive(Error, Debug)]
pub enum MiniDSPError {
    #[error("An HID error has occurred: {0}")]
    #[cfg(feature = "hid")]
    HIDError(#[from] HidError),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("A malformed packet was received: {0}")]
    MalformedResponse(String),

    #[error("This source was not recognized. Supported types are: 'toslink', 'usb', 'analog'")]
    InvalidSource,

    #[error("There are too many coeffiients in this filter")]
    TooManyCoefficients,

    #[error("Parse error")]
    ParseError(#[from] minidsp_protocol::ParseError),

    #[error("Malformed filter data")]
    MalformedFilterData,

    #[error("Transport error")]
    TransportError(#[from] broadcast::error::RecvError),

    #[error("Transport error: {0}")]
    TransportFailure(String),

    #[error("Transport has closed")]
    TransportClosed,

    #[error("WebSocket transport error: {0}")]
    WebSocketError(#[from] ws::Error),

    #[error("Multiple concurrent commands were sent")]
    ConcurencyError,

    #[error("Internal error")]
    InternalError(#[from] anyhow::Error),

    #[error("Specified channel or peq is out of range")]
    OutOfRange,

    #[error("The specified URL was invalid")]
    InvalidURL,

    #[error("Protocol error: {0}")]
    ProtocolError(#[from] ProtocolError),

    #[error("Url error: {0}")]
    Url2Error(#[from] url2::Url2Error),

    #[error("This device does not have this peripheral")]
    NoSuchPeripheral,

    #[error("A device request timed out")]
    Timeout,
}

#[async_trait]
pub trait Openable {
    async fn open(&self) -> Result<Transport, MiniDSPError>;
    fn to_url(&self) -> String;
}

pub trait IntoTransport {
    fn into_transport(self) -> Transport;
}

pub async fn open_url(url: &Url2) -> Result<Transport, MiniDSPError> {
    match url.scheme() {
        #[cfg(feature = "hid")]
        "usb" => {
            let api = hid::initialize_api()?;
            Ok(hid::HidTransport::with_url(&api, url)
                .map_err(MiniDSPError::HIDError)?
                .into_transport())
        }
        "tcp" => Ok(net::open_url(url).await?.into_transport()),
        "ws" | "wss" => Ok(ws::open_url(url).await?),
        _ => Err(MiniDSPError::InvalidURL),
    }
}

#[async_trait]
impl Openable for Url2 {
    async fn open(&self) -> Result<Transport, MiniDSPError> {
        open_url(self).await
    }

    fn to_url(&self) -> String {
        self.to_string()
    }
}
