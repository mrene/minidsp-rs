//! Custom names for each device's input and output channels, shared by every client.
//! Stored in `channel-names.json`, next to the preset library, keyed by device serial number.
use std::{collections::BTreeMap, path::PathBuf};

use anyhow::Context;
use hyper::{Body, Request, Response};
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    get_device,
    helpers::{parse_body, parse_param, serialize_response},
    presets::presets_dir,
    Error,
};

/// Longest accepted channel name
const MAX_LEN: usize = 32;

/// Channel names of one device, by 0-based channel index
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ChannelNames {
    #[serde(default)]
    pub inputs: BTreeMap<usize, String>,
    #[serde(default)]
    pub outputs: BTreeMap<usize, String>,
}

/// Serializes read-modify-write cycles on the names file
static LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));

async fn names_file() -> PathBuf {
    let presets = presets_dir().await;
    presets
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or(presets)
        .join("channel-names.json")
}

async fn read_all(path: &PathBuf) -> Result<BTreeMap<String, ChannelNames>, Error> {
    match tokio::fs::read(path).await {
        Ok(data) => Ok(serde_json::from_slice(&data)
            .with_context(|| format!("{} is not valid", path.display()))?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(anyhow::Error::from(e)
            .context(format!("cannot read {}", path.display()))
            .into()),
    }
}

/// Identifies a device across restarts: its serial number, or its URL if that isn't known
fn device_key(app: &crate::App, index: usize) -> Result<(String, usize, usize), Error> {
    let device = get_device(app, index)?;
    let spec = device.device_spec().ok_or(Error::DeviceNotReady)?;
    let key = match device.device_info() {
        Some(info) => format!("serial:{}", info.serial),
        None => format!("url:{}", device.url),
    };
    Ok((key, spec.inputs.len(), spec.outputs.len()))
}

pub async fn get(req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let (key, _, _) = {
        let app = super::super::APP.get().unwrap().read().await;
        device_key(&app, device_index)?
    };
    let names = read_all(&names_file().await)
        .await?
        .remove(&key)
        .unwrap_or_default();
    Ok(serialize_response(&req, names)?)
}

/// Replaces all channel names of a device. Empty names are removed.
pub async fn put(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let (key, inputs, outputs) = {
        let app = super::super::APP.get().unwrap().read().await;
        device_key(&app, device_index)?
    };
    let requested: ChannelNames = parse_body(&mut req).await?;

    let clean = |names: BTreeMap<usize, String>, count: usize, kind: &str| {
        let mut out = BTreeMap::new();
        for (index, name) in names {
            if index >= count {
                return Err(Error::parameter_error(
                    kind,
                    format!(
                        "this device has {count} {kind}, there is no channel {}",
                        index + 1
                    ),
                ));
            }
            let name = name.trim();
            if name.chars().count() > MAX_LEN {
                return Err(Error::parameter_error(
                    kind,
                    format!("names can be at most {MAX_LEN} characters"),
                ));
            }
            if !name.is_empty() {
                out.insert(index, name.to_string());
            }
        }
        Ok(out)
    };
    let names = ChannelNames {
        inputs: clean(requested.inputs, inputs, "inputs")?,
        outputs: clean(requested.outputs, outputs, "outputs")?,
    };

    let _guard = LOCK.lock().await;
    let path = names_file().await;
    let mut all = read_all(&path).await?;
    if names.inputs.is_empty() && names.outputs.is_empty() {
        all.remove(&key);
    } else {
        all.insert(key, names.clone());
    }

    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(&all).context("cannot serialize channel names")?;
    tokio::fs::write(&tmp, data)
        .await
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    tokio::fs::rename(&tmp, &path)
        .await
        .with_context(|| format!("cannot write {}", path.display()))?;

    Ok(serialize_response(&req, names)?)
}
