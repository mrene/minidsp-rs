//! Locks that protect the input source and the active preset from accidental changes.
//! They're enforced here, so they apply to every client (browsers, the CLI over HTTP, MCP).
//! Stored in `locks.json`, next to the preset library, keyed by device serial number.
use hyper::{Body, Request, Response};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    device_store::{self, device_key},
    helpers::{parse_body, parse_param, serialize_response},
    Error,
};

const FILE: &str = "locks.json";

fn locked() -> bool {
    true
}

/// Both locks are on until unlocked, so nothing changes by accident
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Locks {
    /// Refuse changes to the input source
    #[serde(default = "locked")]
    pub source: bool,
    /// Refuse switching the active preset (configuration slot) or loading presets into slots
    #[serde(default = "locked")]
    pub preset: bool,
}

impl Default for Locks {
    fn default() -> Self {
        Self {
            source: true,
            preset: true,
        }
    }
}

pub async fn load(device_index: usize) -> Result<Locks, Error> {
    let device = device_key(device_index).await?;
    device_store::load(FILE, &device.key).await
}

/// Fails if a requested change touches something that is locked. Call before taking the
/// application lock, since it reads the device's identity.
pub async fn check(device_index: usize, source: bool, preset: bool) -> Result<(), Error> {
    if !source && !preset {
        return Ok(());
    }
    let locks = load(device_index).await?;
    if source && locks.source {
        return Err(Error::Locked {
            what: "The input source".into(),
        });
    }
    if preset && locks.preset {
        return Err(Error::Locked {
            what: "The active preset".into(),
        });
    }
    Ok(())
}

pub async fn get(req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    Ok(serialize_response(&req, load(device_index).await?)?)
}

pub async fn put(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let device = device_key(device_index).await?;
    let locks: Locks = parse_body(&mut req).await?;
    device_store::store(FILE, &device.key, Some(&locks)).await?;
    Ok(serialize_response(&req, locks)?)
}
