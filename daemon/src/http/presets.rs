//! A library of presets stored as JSON files, which can be written into one of the device's
//! configuration slots. The device only has a few slots, the library can hold any number of
//! presets.
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context};
use hyper::{Body, Request, Response, StatusCode};
use minidsp::model::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    get_device, get_device_instance,
    helpers::{parse_body, parse_param, serialize_response},
    Error,
};

/// A preset as stored on disk
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Preset {
    pub name: String,

    #[serde(default)]
    pub notes: String,

    /// Product this preset was made for (e.g. "2x4HD"); presets only apply to that product
    pub product_name: String,

    /// Last modification time, in milliseconds since the Unix epoch (set by the server)
    #[serde(default)]
    pub updated_ms: u64,

    /// Settings written to the device when the preset is loaded
    pub config: Config,

    /// Editable parameters for user interfaces (e.g. filter frequency and Q, rather than
    /// coefficients). Opaque to the daemon.
    #[serde(default)]
    #[schemars(with = "serde_json::Value")]
    pub ui: serde_json::Value,
}

/// A preset listing entry, without its settings
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct PresetSummary {
    pub id: String,
    pub name: String,
    pub notes: String,
    pub product_name: String,
    pub updated_ms: u64,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct PresetId {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ApplyRequest {
    /// Device configuration slot (0-based) to write the preset into
    pub slot: u8,
}

/// Where presets are stored: the configured directory, or a per-user default
async fn presets_dir() -> PathBuf {
    let app = super::super::APP.get().unwrap().read().await;
    if let Some(dir) = &app.config.presets_dir {
        return PathBuf::from(dir);
    }
    match std::env::var_os("HOME") {
        Some(home) if !home.is_empty() => PathBuf::from(home).join(".local/share/minidsp/presets"),
        _ => PathBuf::from("/var/lib/minidsp/presets"),
    }
}

/// Preset ids are file names, so only allow a safe subset of characters
fn validate_id(id: &str) -> Result<(), Error> {
    let valid = !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if valid {
        Ok(())
    } else {
        Err(Error::parameter_error("id", "invalid preset id"))
    }
}

fn preset_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

/// Builds an id from the preset name, with a time-based suffix to keep it unique
fn new_id(name: &str) -> String {
    let mut slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    let slug: String = slug.chars().take(48).collect();
    let slug = if slug.is_empty() { "preset" } else { &slug };
    format!("{slug}-{:x}", now_ms())
}

async fn read_preset(dir: &Path, id: &str) -> Result<Preset, Error> {
    let path = preset_path(dir, id);
    let data = tokio::fs::read(&path)
        .await
        .with_context(|| format!("preset '{id}' not found"))?;
    Ok(serde_json::from_slice(&data).with_context(|| format!("preset '{id}' is not valid"))?)
}

/// Writes atomically, so a crash never leaves a half-written preset behind
async fn write_preset(dir: &Path, id: &str, preset: &Preset) -> Result<(), Error> {
    tokio::fs::create_dir_all(dir)
        .await
        .with_context(|| format!("cannot create presets directory {}", dir.display()))?;
    let path = preset_path(dir, id);
    let tmp = dir.join(format!(".{id}.json.tmp"));
    let data = serde_json::to_vec_pretty(preset).context("cannot serialize preset")?;
    tokio::fs::write(&tmp, data)
        .await
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    tokio::fs::rename(&tmp, &path)
        .await
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

fn validate_preset(preset: &Preset) -> Result<(), Error> {
    if preset.name.trim().is_empty() {
        return Err(Error::ParseError {
            reason: "the preset needs a name".into(),
        });
    }
    if preset.product_name.trim().is_empty() {
        return Err(Error::ParseError {
            reason: "the preset needs a product name".into(),
        });
    }
    Ok(())
}

/// Lists the presets in the library, most recently updated first
pub async fn list(req: Request<Body>) -> Result<Response<Body>, Error> {
    let dir = presets_dir().await;
    let mut presets = Vec::new();

    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(entries) => entries,
        // No presets saved yet
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(serialize_response(&req, presets)?);
        }
        Err(e) => return Err(anyhow!(e).context("cannot read presets directory").into()),
    };

    while let Some(entry) = entries
        .next_entry()
        .await
        .context("cannot read presets directory")?
    {
        let path = entry.path();
        let Some(id) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".json"))
        else {
            continue;
        };
        if validate_id(id).is_err() {
            continue;
        }
        match read_preset(&dir, id).await {
            Ok(p) => presets.push(PresetSummary {
                id: id.to_string(),
                name: p.name,
                notes: p.notes,
                product_name: p.product_name,
                updated_ms: p.updated_ms,
            }),
            Err(e) => log::warn!("skipping preset {}: {e}", path.display()),
        }
    }

    presets.sort_by(|a, b| b.updated_ms.cmp(&a.updated_ms));
    Ok(serialize_response(&req, presets)?)
}

pub async fn get(req: Request<Body>) -> Result<Response<Body>, Error> {
    let id: String = parse_param(&req, "presetId")?;
    validate_id(&id)?;
    let preset = read_preset(&presets_dir().await, &id).await?;
    Ok(serialize_response(&req, preset)?)
}

/// Saves a new preset and returns its id
pub async fn create(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let mut preset: Preset = parse_body(&mut req).await?;
    validate_preset(&preset)?;
    preset.updated_ms = now_ms();
    let id = new_id(&preset.name);
    write_preset(&presets_dir().await, &id, &preset).await?;
    Ok(serialize_response(&req, PresetId { id })?)
}

/// Replaces an existing preset
pub async fn update(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let id: String = parse_param(&req, "presetId")?;
    validate_id(&id)?;
    let dir = presets_dir().await;
    // Only update presets that exist, creating is done through `create`
    read_preset(&dir, &id).await?;

    let mut preset: Preset = parse_body(&mut req).await?;
    validate_preset(&preset)?;
    preset.updated_ms = now_ms();
    write_preset(&dir, &id, &preset).await?;
    Ok(serialize_response(&req, PresetId { id })?)
}

pub async fn delete(req: Request<Body>) -> Result<Response<Body>, Error> {
    let id: String = parse_param(&req, "presetId")?;
    validate_id(&id)?;
    let path = preset_path(&presets_dir().await, &id);
    tokio::fs::remove_file(&path)
        .await
        .with_context(|| format!("preset '{id}' not found"))?;
    Ok(Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .map_err(|e| Error::from(anyhow!(e)))?)
}

/// Switches the device to a configuration slot, then writes the preset's settings into it
pub async fn apply(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let id: String = parse_param(&req, "presetId")?;
    validate_id(&id)?;
    let body: ApplyRequest = parse_body(&mut req).await?;
    // miniDSP devices have 4 configuration slots
    if body.slot > 3 {
        return Err(Error::parameter_error("slot", "must be between 0 and 3"));
    }
    let preset = read_preset(&presets_dir().await, &id).await?;

    let app = super::super::APP.get().unwrap();
    let app = app.read().await;
    let product = get_device(&app, device_index)?
        .device_spec()
        .map(|s| s.product_name)
        .ok_or(Error::DeviceNotReady)?;
    if product != preset.product_name {
        return Err(Error::ParseError {
            reason: format!(
                "preset '{}' was made for a {}, this device is a {product}",
                preset.name, preset.product_name
            ),
        });
    }

    let device = get_device_instance(&app, device_index).await?;
    device.set_config(body.slot).await?;
    // Global settings (volume, source) are not part of a preset
    let config = Config {
        master_status: None,
        ..preset.config
    };
    config.apply(&device).await?;

    let status = device.get_master_status().await?;
    Ok(serialize_response(&req, status)?)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ids_are_safe_file_names() {
        assert!(validate_id("living-room-18c3f2").is_ok());
        assert!(validate_id("../etc/passwd").is_err());
        assert!(validate_id("a/b").is_err());
        assert!(validate_id("UPPER").is_err());
        assert!(validate_id("").is_err());

        let id = new_id("Living Room: Sub + Mains (v2)");
        assert!(id.starts_with("living-room-sub-mains-v2-"), "{id}");
        assert!(validate_id(&id).is_ok());
        assert!(validate_id(&new_id("???")).is_ok());
    }
}
