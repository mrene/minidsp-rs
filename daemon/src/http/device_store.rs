//! Small per-device JSON stores (channel names, channel links), kept next to the preset library
//! and keyed by device serial number, so settings follow the device across restarts.
use std::{collections::BTreeMap, path::PathBuf};

use anyhow::Context;
use once_cell::sync::Lazy;
use serde::{de::DeserializeOwned, Serialize};

use super::{get_device, presets::presets_dir, Error};

/// Serializes read-modify-write cycles on the store files
static LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));

/// A device's identity and channel counts
pub struct DeviceKey {
    pub key: String,
    pub inputs: usize,
    pub outputs: usize,
}

/// Identifies a device across restarts: its serial number, or its URL if that isn't known
pub async fn device_key(index: usize) -> Result<DeviceKey, Error> {
    let app = super::super::APP.get().unwrap().read().await;
    let device = get_device(&app, index)?;
    let spec = device.device_spec().ok_or(Error::DeviceNotReady)?;
    let key = match device.device_info() {
        Some(info) => format!("serial:{}", info.serial),
        None => format!("url:{}", device.url),
    };
    Ok(DeviceKey {
        key,
        inputs: spec.inputs.len(),
        outputs: spec.outputs.len(),
    })
}

async fn file(name: &str) -> PathBuf {
    let presets = presets_dir().await;
    presets
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or(presets)
        .join(name)
}

async fn read_all<T: DeserializeOwned>(path: &PathBuf) -> Result<BTreeMap<String, T>, Error> {
    match tokio::fs::read(path).await {
        Ok(data) => Ok(serde_json::from_slice(&data)
            .with_context(|| format!("{} is not valid", path.display()))?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(anyhow::Error::from(e)
            .context(format!("cannot read {}", path.display()))
            .into()),
    }
}

/// Reads a device's entry from a store file, or the default if it has none
pub async fn load<T: DeserializeOwned + Default>(file_name: &str, key: &str) -> Result<T, Error> {
    Ok(read_all::<T>(&file(file_name).await)
        .await?
        .remove(key)
        .unwrap_or_default())
}

/// Replaces a device's entry in a store file; `None` removes it
pub async fn store<T: Serialize + DeserializeOwned>(
    file_name: &str,
    key: &str,
    value: Option<&T>,
) -> Result<(), Error> {
    let _guard = LOCK.lock().await;
    let path = file(file_name).await;
    let mut all = read_all::<serde_json::Value>(&path).await?;
    match value {
        Some(v) => {
            all.insert(
                key.to_string(),
                serde_json::to_value(v).context("cannot serialize")?,
            );
        }
        None => {
            all.remove(key);
        }
    }

    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(&all).context("cannot serialize")?;
    tokio::fs::write(&tmp, data)
        .await
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    tokio::fs::rename(&tmp, &path)
        .await
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}
