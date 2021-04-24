//! Transport base traits for talking to devices

//! Wraps a Stream + Sink backend into a transport
use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use commands::Commands;
use futures::future::BoxFuture;
use minidsp_protocol::commands::ProtocolError;
use thiserror::Error;
use tokio::sync::{broadcast, Mutex};
use tower::Service;
use url2::Url2;

pub type SharedService = Arc<
    Mutex<
        dyn Service<
                Commands,
                Response = Responses,
                Error = MiniDSPError,
                Future = BoxFuture<'static, Result<Responses, MiniDSPError>>,
            > + Send,
    >,
>;

pub type Transport =
    Pin<Box<dyn StreamSink<'static, Result<Bytes, MiniDSPError>, Bytes, MiniDSPError> + Send>>;

#[cfg(feature = "hid")]
pub mod hid;

#[cfg(feature = "hid")]
use hidapi::HidError;

use crate::{
    commands::{self, Responses},
    utils::StreamSink,
};

pub mod frame_codec;
pub mod multiplexer;
pub use multiplexer::Multiplexer;
pub mod hub;
pub use hub::Hub;
pub mod net;

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

    #[error("This device does not have this peripheral")]
    NoSuchPeripheral,
}

#[async_trait]
pub trait Openable {
    async fn open(&self) -> Result<Transport, MiniDSPError>;
    fn to_string(&self) -> String;
}

pub trait IntoTransport {
    fn into_transport(self) -> Transport;
}

pub async fn open_url(url: Url2) -> Result<Transport, MiniDSPError> {
    match url.scheme() {
        #[cfg(feature = "hid")]
        "usb" => {
            let api = hid::initialize_api()?;
            Ok(hid::HidTransport::with_url(&api, url)
                .map_err(MiniDSPError::HIDError)?
                .into_transport())
        }
        "tcp" => Ok(net::open_url(url).await?.into_transport()),
        _ => Err(MiniDSPError::InvalidURL),
    }
}
