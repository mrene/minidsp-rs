//! A library of presets stored as JSON files, which can be written into one of the device's
//! configuration slots. The device only has a few slots, the library can hold any number of
//! presets.
//!
//! Each preset is a file named after the preset (`<name>.json`). Every change is recorded as a
//! numbered version under `.history/<name>/`, which allows undoing changes, rolling back to any
//! version and restoring deleted presets:
//!
//! ```text
//! presets/
//!   Living room.json            current version
//!   .history/Living room/
//!     index.json                one entry per version: number, time, what changed
//!     v0001.json                complete snapshot of each version
//!     v0002.json
//! ```
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context};
use hyper::{Body, Request, Response};
use minidsp::model::Config;
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::{
    get_device, get_device_instance,
    helpers::{parse_body, parse_param, serialize_response},
    Error,
};

/// Longest accepted preset name
const MAX_NAME_LEN: usize = 80;

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

/// A preset sent by a client, with an optional description of the change for the history
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct PresetWrite {
    #[serde(flatten)]
    pub preset: Preset,

    /// What changed, e.g. "updated from slot 2"; shown in the history
    #[serde(default)]
    pub action: Option<String>,
}

/// A preset listing entry, without its settings
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct PresetSummary {
    /// Identifies the preset in URLs; this is its name
    pub id: String,
    pub name: String,
    pub notes: String,
    pub product_name: String,
    pub updated_ms: u64,
    /// Latest version number, 0 if the preset has no history yet
    pub version: u32,
}

/// One recorded version of a preset
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct HistoryEntry {
    pub version: u32,
    /// When the version was recorded, in milliseconds since the Unix epoch
    pub at_ms: u64,
    /// What changed, e.g. "created", "renamed from …", "rolled back to version 3"
    pub action: String,
    /// The preset's name at that version
    pub name: String,
    /// For undo and rollback: the version whose settings were restored
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restored_from: Option<u32>,
    /// Set on the version recorded when the preset was deleted
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
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

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RenameRequest {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RollbackRequest {
    pub version: u32,
}

/// Serializes every change to the library, so concurrent requests can't interleave
static LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));

/// Where presets are stored: the configured directory, or a per-user default
pub(super) async fn presets_dir() -> PathBuf {
    let app = super::super::APP.get().unwrap().read().await;
    if let Some(dir) = &app.config.presets_dir {
        return PathBuf::from(dir);
    }
    match std::env::var_os("HOME") {
        Some(home) if !home.is_empty() => PathBuf::from(home).join(".local/share/minidsp/presets"),
        _ => PathBuf::from("/var/lib/minidsp/presets"),
    }
}

fn bad_request(reason: impl Into<String>) -> Error {
    Error::ParseError {
        reason: reason.into(),
    }
}

/// Preset names are also file names, so refuse anything that could escape the presets
/// directory or that file systems reject
fn validate_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() {
        return Err(bad_request("the preset needs a name"));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(bad_request(format!(
            "preset names can be at most {MAX_NAME_LEN} characters"
        )));
    }
    if name.starts_with('.') {
        return Err(bad_request("preset names can't start with a dot"));
    }
    if let Some(c) = name.chars().find(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    }) {
        return Err(bad_request(format!(
            "preset names can't contain '{}'",
            c.escape_default()
        )));
    }
    Ok(name.to_string())
}

/// Decodes `%XX` escapes in a path parameter (preset names may contain spaces and symbols)
fn percent_decode(s: &str) -> Result<String, Error> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).map_err(|_| Error::parameter_error("presetId", "invalid preset name"))
}

/// Reads the preset name from the URL
fn id_param(req: &Request<Body>) -> Result<String, Error> {
    let raw: String = parse_param(req, "presetId")?;
    let name = percent_decode(&raw)?;
    validate_name(&name).map_err(|_| Error::parameter_error("presetId", "invalid preset name"))?;
    if name.trim() != name {
        return Err(Error::parameter_error("presetId", "invalid preset name"));
    }
    Ok(name)
}

fn current_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

fn history_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join(".history").join(id)
}

fn snapshot_path(dir: &Path, id: &str, version: u32) -> PathBuf {
    history_dir(dir, id).join(format!("v{version:04}.json"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

async fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, Error> {
    match tokio::fs::read(path).await {
        Ok(data) => {
            Ok(Some(serde_json::from_slice(&data).with_context(|| {
                format!("{} is not valid", path.display())
            })?))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow!(e)
            .context(format!("cannot read {}", path.display()))
            .into()),
    }
}

/// Writes atomically, so a crash never leaves a half-written file behind
async fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), Error> {
    let parent = path.parent().context("invalid path")?;
    tokio::fs::create_dir_all(parent)
        .await
        .with_context(|| format!("cannot create {}", parent.display()))?;
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(value).context("cannot serialize")?;
    tokio::fs::write(&tmp, data)
        .await
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    tokio::fs::rename(&tmp, path)
        .await
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

async fn read_preset(dir: &Path, id: &str) -> Result<Preset, Error> {
    read_json(&current_path(dir, id))
        .await?
        .ok_or_else(|| bad_request(format!("there is no preset named “{id}”")))
}

async fn read_index(dir: &Path, id: &str) -> Result<Vec<HistoryEntry>, Error> {
    Ok(read_json(&history_dir(dir, id).join("index.json"))
        .await?
        .unwrap_or_default())
}

/// Names of current presets (file stems), for case-insensitive uniqueness checks
async fn existing_names(dir: &Path) -> Result<Vec<String>, Error> {
    let mut names = Vec::new();
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(names),
        Err(e) => return Err(anyhow!(e).context("cannot read presets directory").into()),
    };
    while let Some(entry) = entries
        .next_entry()
        .await
        .context("cannot read presets directory")?
    {
        if let Some(stem) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.strip_suffix(".json"))
        {
            if !stem.starts_with('.') {
                names.push(stem.to_string());
            }
        }
    }
    Ok(names)
}

async fn name_taken(dir: &Path, name: &str, except: Option<&str>) -> Result<bool, Error> {
    let lower = name.to_lowercase();
    Ok(existing_names(dir)
        .await?
        .iter()
        .any(|n| n.to_lowercase() == lower && Some(n.as_str()) != except))
}

/// Records a new version: a full snapshot plus an index entry. Presets saved before history
/// existed get their previous state recorded first, so nothing is lost.
async fn record(
    dir: &Path,
    id: &str,
    preset: &Preset,
    action: String,
    restored_from: Option<u32>,
    deleted: bool,
) -> Result<HistoryEntry, Error> {
    let mut index = read_index(dir, id).await?;
    if index.is_empty() {
        if let Some(previous) = read_json::<Preset>(&current_path(dir, id)).await? {
            write_json(&snapshot_path(dir, id, 1), &previous).await?;
            index.push(HistoryEntry {
                version: 1,
                at_ms: previous.updated_ms,
                action: "saved before history was kept".into(),
                name: previous.name.clone(),
                restored_from: None,
                deleted: false,
            });
        }
    }

    let entry = HistoryEntry {
        version: index.last().map_or(1, |e| e.version + 1),
        at_ms: preset.updated_ms,
        action,
        name: preset.name.clone(),
        restored_from,
        deleted,
    };
    write_json(&snapshot_path(dir, id, entry.version), preset).await?;
    index.push(entry.clone());
    write_json(&history_dir(dir, id).join("index.json"), &index).await?;
    Ok(entry)
}

fn summary(id: String, preset: &Preset, version: u32) -> PresetSummary {
    PresetSummary {
        id,
        name: preset.name.clone(),
        notes: preset.notes.clone(),
        product_name: preset.product_name.clone(),
        updated_ms: preset.updated_ms,
        version,
    }
}

fn validate_preset(preset: &Preset) -> Result<(), Error> {
    if preset.product_name.trim().is_empty() {
        return Err(bad_request("the preset needs a product name"));
    }
    Ok(())
}

// ---- Endpoints -----------------------------------------------------------------------------

/// Lists the presets in the library, most recently updated first
pub async fn list(req: Request<Body>) -> Result<Response<Body>, Error> {
    let dir = presets_dir().await;
    let mut presets = Vec::new();
    for id in existing_names(&dir).await? {
        match read_preset(&dir, &id).await {
            Ok(p) => {
                let version = read_index(&dir, &id).await?.last().map_or(0, |e| e.version);
                presets.push(summary(id, &p, version));
            }
            Err(e) => log::warn!("skipping preset {id}: {e}"),
        }
    }
    presets.sort_by(|a, b| b.updated_ms.cmp(&a.updated_ms));
    Ok(serialize_response(&req, presets)?)
}

/// Lists deleted presets that can be restored, most recently deleted first
pub async fn list_deleted(req: Request<Body>) -> Result<Response<Body>, Error> {
    let dir = presets_dir().await;
    let current: Vec<String> = existing_names(&dir).await?;
    let mut deleted = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(dir.join(".history")).await {
        while let Some(entry) = entries.next_entry().await.context("cannot read history")? {
            let Some(id) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if current.contains(&id) {
                continue;
            }
            let index = read_index(&dir, &id).await?;
            if let Some(last) = index.last().filter(|e| e.deleted) {
                if let Some(p) =
                    read_json::<Preset>(&snapshot_path(&dir, &id, last.version)).await?
                {
                    let mut s = summary(id, &p, last.version);
                    s.updated_ms = last.at_ms;
                    deleted.push(s);
                }
            }
        }
    }
    deleted.sort_by(|a, b| b.updated_ms.cmp(&a.updated_ms));
    Ok(serialize_response(&req, deleted)?)
}

pub async fn get(req: Request<Body>) -> Result<Response<Body>, Error> {
    let id = id_param(&req)?;
    let preset = read_preset(&presets_dir().await, &id).await?;
    Ok(serialize_response(&req, preset)?)
}

/// Saves a new preset, named and stored after its `name`
pub async fn create(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let PresetWrite { mut preset, action } = parse_body(&mut req).await?;
    validate_preset(&preset)?;
    let name = validate_name(&preset.name)?;
    preset.name = name.clone();
    preset.updated_ms = now_ms();

    let _guard = LOCK.lock().await;
    let dir = presets_dir().await;
    if name_taken(&dir, &name, None).await? {
        return Err(bad_request(format!(
            "a preset named “{name}” already exists"
        )));
    }
    // A deleted preset with this name keeps its history; the new preset continues it
    let replaces_deleted = !read_index(&dir, &name).await?.is_empty();
    let action = action.unwrap_or_else(|| "created".into());
    let action = if replaces_deleted {
        format!("{action} (reusing the name of a deleted preset)")
    } else {
        action
    };
    record(&dir, &name, &preset, action, None, false).await?;
    write_json(&current_path(&dir, &name), &preset).await?;
    Ok(serialize_response(&req, PresetId { id: name })?)
}

/// Replaces a preset's settings and notes. Renaming is done through `rename`.
pub async fn update(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let id = id_param(&req)?;
    let PresetWrite { mut preset, action } = parse_body(&mut req).await?;
    validate_preset(&preset)?;

    let _guard = LOCK.lock().await;
    let dir = presets_dir().await;
    read_preset(&dir, &id).await?;
    preset.name = id.clone();
    preset.updated_ms = now_ms();
    record(
        &dir,
        &id,
        &preset,
        action.unwrap_or_else(|| "saved".into()),
        None,
        false,
    )
    .await?;
    write_json(&current_path(&dir, &id), &preset).await?;
    Ok(serialize_response(&req, PresetId { id })?)
}

/// Renames a preset: its file and history move to the new name
pub async fn rename(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let id = id_param(&req)?;
    let RenameRequest { name } = parse_body(&mut req).await?;
    let name = validate_name(&name)?;

    let _guard = LOCK.lock().await;
    let dir = presets_dir().await;
    let mut preset = read_preset(&dir, &id).await?;
    if name == id {
        return Ok(serialize_response(&req, PresetId { id })?);
    }
    if name_taken(&dir, &name, Some(&id)).await? {
        return Err(bad_request(format!(
            "a preset named “{name}” already exists"
        )));
    }
    let case_only = name.to_lowercase() == id.to_lowercase();
    if !case_only && tokio::fs::metadata(history_dir(&dir, &name)).await.is_ok() {
        return Err(bad_request(format!(
            "a deleted preset is named “{name}”; restore it or pick another name"
        )));
    }

    // Make sure the history holds the current version before moving it
    if read_index(&dir, &id).await?.is_empty() {
        record(
            &dir,
            &id,
            &preset,
            "saved before history was kept".into(),
            None,
            false,
        )
        .await?;
    }
    let move_path = |from: PathBuf, to: PathBuf| async move {
        tokio::fs::rename(&from, &to)
            .await
            .with_context(|| format!("cannot rename {} to {}", from.display(), to.display()))
    };
    move_path(history_dir(&dir, &id), history_dir(&dir, &name)).await?;
    move_path(current_path(&dir, &id), current_path(&dir, &name)).await?;

    preset.name = name.clone();
    preset.updated_ms = now_ms();
    record(
        &dir,
        &name,
        &preset,
        format!("renamed from “{id}”"),
        None,
        false,
    )
    .await?;
    write_json(&current_path(&dir, &name), &preset).await?;
    Ok(serialize_response(&req, PresetId { id: name })?)
}

/// Lists a preset's versions, newest first
pub async fn history(req: Request<Body>) -> Result<Response<Body>, Error> {
    let id = id_param(&req)?;
    let dir = presets_dir().await;
    let mut index = read_index(&dir, &id).await?;
    if index.is_empty() {
        // Saved before history was kept: report the current state as the only version
        let preset = read_preset(&dir, &id).await?;
        index.push(HistoryEntry {
            version: 1,
            at_ms: preset.updated_ms,
            action: "saved before history was kept".into(),
            name: preset.name,
            restored_from: None,
            deleted: false,
        });
    }
    index.reverse();
    Ok(serialize_response(&req, index)?)
}

/// Restores the settings of an earlier version, recorded as a new version
async fn restore_version(
    dir: &Path,
    id: &str,
    version: u32,
    action: String,
) -> Result<HistoryEntry, Error> {
    let mut preset: Preset = read_json(&snapshot_path(dir, id, version))
        .await?
        .ok_or_else(|| bad_request(format!("“{id}” has no version {version}")))?;
    // The name stays the current one, even if the version predates a rename
    preset.name = id.to_string();
    preset.updated_ms = now_ms();
    let entry = record(dir, id, &preset, action, Some(version), false).await?;
    write_json(&current_path(dir, id), &preset).await?;
    Ok(entry)
}

/// Rolls a preset back to any earlier version
pub async fn rollback(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let id = id_param(&req)?;
    let RollbackRequest { version } = parse_body(&mut req).await?;
    let _guard = LOCK.lock().await;
    let dir = presets_dir().await;
    read_preset(&dir, &id).await?;
    let entry = restore_version(
        &dir,
        &id,
        version,
        format!("rolled back to version {version}"),
    )
    .await?;
    Ok(serialize_response(&req, entry)?)
}

/// Undoes the last change. Repeating it keeps stepping back through earlier versions.
pub async fn undo(req: Request<Body>) -> Result<Response<Body>, Error> {
    let id = id_param(&req)?;
    let _guard = LOCK.lock().await;
    let dir = presets_dir().await;
    read_preset(&dir, &id).await?;
    let index = read_index(&dir, &id).await?;
    let target = undo_target(&index).ok_or_else(|| bad_request("there is nothing to undo"))?;
    let entry =
        restore_version(&dir, &id, target, format!("undo, back to version {target}")).await?;
    Ok(serialize_response(&req, entry)?)
}

/// The version an undo goes back to: the one before the last change. When the last change was
/// itself an undo or rollback, step back from the version it restored.
fn undo_target(index: &[HistoryEntry]) -> Option<u32> {
    let last = index.last()?;
    let effective = last.restored_from.unwrap_or(last.version);
    (effective > 1).then(|| effective - 1)
}

/// Deletes a preset. Its history is kept, so it can be restored.
pub async fn delete(req: Request<Body>) -> Result<Response<Body>, Error> {
    let id = id_param(&req)?;
    let _guard = LOCK.lock().await;
    let dir = presets_dir().await;
    let mut preset = read_preset(&dir, &id).await?;
    preset.updated_ms = now_ms();
    record(&dir, &id, &preset, "deleted".into(), None, true).await?;
    tokio::fs::remove_file(current_path(&dir, &id))
        .await
        .with_context(|| format!("cannot delete “{id}”"))?;
    Ok(serialize_response(&req, PresetId { id })?)
}

/// Restores a deleted preset as it was when deleted
pub async fn restore(req: Request<Body>) -> Result<Response<Body>, Error> {
    let id = id_param(&req)?;
    let _guard = LOCK.lock().await;
    let dir = presets_dir().await;
    if name_taken(&dir, &id, None).await? {
        return Err(bad_request(format!("a preset named “{id}” already exists")));
    }
    let index = read_index(&dir, &id).await?;
    let last = index
        .last()
        .filter(|e| e.deleted)
        .ok_or_else(|| bad_request(format!("“{id}” is not a deleted preset")))?;
    let entry = restore_version(&dir, &id, last.version, "restored after deletion".into()).await?;
    Ok(serialize_response(&req, entry)?)
}

/// Switches the device to a configuration slot, then writes the preset's settings into it
pub async fn apply(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let id = id_param(&req)?;
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
        return Err(bad_request(format!(
            "preset “{}” was made for a {}, this device is a {product}",
            preset.name, preset.product_name
        )));
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

    fn entry(version: u32, restored_from: Option<u32>) -> HistoryEntry {
        HistoryEntry {
            version,
            at_ms: 0,
            action: String::new(),
            name: String::new(),
            restored_from,
            deleted: false,
        }
    }

    #[test]
    fn names_are_safe_file_names() {
        assert_eq!(
            validate_name("  Living room (v2) ").unwrap(),
            "Living room (v2)"
        );
        assert!(validate_name("Sub + mains, 80 Hz").is_ok());
        assert!(validate_name("../etc/passwd").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("a\\b").is_err());
        assert!(validate_name(".hidden").is_err());
        assert!(validate_name("").is_err());
        assert!(validate_name("tab\there").is_err());
        assert!(validate_name(&"x".repeat(81)).is_err());
    }

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(
            percent_decode("Living%20room%20%28v2%29").unwrap(),
            "Living room (v2)"
        );
        assert_eq!(percent_decode("caf%C3%A9").unwrap(), "café");
        assert_eq!(percent_decode("100%").unwrap(), "100%");
        // A decoded slash is still rejected by validate_name
        assert!(validate_name(&percent_decode("..%2Fetc").unwrap()).is_err());
    }

    #[test]
    fn undo_steps_back_through_versions() {
        // v1 created, v2 saved, v3 saved
        let mut index = vec![entry(1, None), entry(2, None), entry(3, None)];
        assert_eq!(undo_target(&index), Some(2));
        // v4 = undo back to v2; undoing again goes to v1
        index.push(entry(4, Some(2)));
        assert_eq!(undo_target(&index), Some(1));
        // v5 = undo back to v1; nothing older
        index.push(entry(5, Some(1)));
        assert_eq!(undo_target(&index), None);
        // A new save after undos can be undone to the restored state
        index.push(entry(6, None));
        assert_eq!(undo_target(&index), Some(5));
        assert_eq!(undo_target(&[entry(1, None)]), None);
    }
}
