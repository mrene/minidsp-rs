//! Helper module to control the minidsp's source and volume based on an RAII guard
//!
use crate::{Gain, MiniDSP, Source};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

/// Returns an RAII guard object setting the source back to what it previously was once its released
pub async fn lease_source(
    minidsp: Arc<Mutex<MiniDSP<'static>>>,
    source: Source,
) -> Result<SourceLease> {
    {
        let minidsp = minidsp.lock().await;
        minidsp.set_source(source).await?;
    }

    let (tx, rx) = oneshot::channel::<()>();
    {
        let minidsp = minidsp.clone();
        tokio::spawn(async move {
            // Wait for the guard object to be dropped
            let _ = rx.await;

            let minidsp = minidsp.lock_owned().await;
            if let Err(e) = minidsp.set_source(Source::Toslink).await {
                eprintln!("Failed to set source back: {:?}", e)
            }
            if let Err(e) = minidsp.set_master_volume(Gain(-40.)).await {
                eprintln!("Failed to set volume back: {:?}", e)
            }
        });
    }

    Ok(SourceLease { tx })
}

/// A RAII guard object causing the source to change back once it's released
pub struct SourceLease {
    #[allow(dead_code)]
    tx: oneshot::Sender<()>,
}
