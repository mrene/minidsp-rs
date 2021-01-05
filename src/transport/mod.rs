//! Transport base traits for talking to devices
//!
use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::ops::DerefMut;
use thiserror::Error;
use tokio::sync::{broadcast, OwnedMutexGuard};

#[cfg(feature = "hid")]
pub mod hid;

#[cfg(feature = "hid")]
use hidapi::HidError;

use crate::commands;

pub mod net;

#[derive(Error, Debug)]
pub enum MiniDSPError {
    #[error("An HID error has occurred: {0}")]
    #[cfg(feature = "hid")]
    HIDError(#[from] HidError),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("A malformed packet was received")]
    MalformedResponse,

    #[error("This source was not recognized. Supported types are: 'toslink', 'usb', 'analog'")]
    InvalidSource,

    #[error("Parse error")]
    ParseError(#[from] commands::ParseError),

    #[error("Transport error")]
    TransportError(#[from] broadcast::error::RecvError),

    #[error("Transport has closed")]
    TransportClosed,

    #[error("Internal error")]
    InternalError(#[from] anyhow::Error),
}

#[async_trait]
pub trait Openable {
    type Transport;
    async fn open(&self) -> Result<Self::Transport, MiniDSPError>;
}

#[async_trait]
impl<T> Sender for OwnedMutexGuard<T>
where
    T: Sender,
{
    async fn send(&mut self, frame: Bytes) -> Result<(), MiniDSPError> {
        // self.deref_mut().send() confuses clion
        T::send(self.deref_mut(), frame).await
    }
}

/// Transport trait implemented by different backends
#[async_trait]
pub trait Transport: Send + Sync {
    // Subscribe to all received frames
    fn subscribe(&self) -> Result<broadcast::Receiver<Bytes>, MiniDSPError>;

    // Acquire an exclusive lock for sending frames on this device
    async fn send_lock(&self) -> Box<dyn Sender>;

    // Sends a single frame
    async fn send(&self, frame: Bytes) -> Result<(), MiniDSPError> {
        let mut tx = self.send_lock().await;
        tx.send(frame).await
    }
}

#[async_trait]
pub trait Sender: Send + Sync {
    async fn send(&mut self, frame: Bytes) -> Result<(), MiniDSPError>;
}
