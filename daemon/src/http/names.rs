//! Custom names for each device's input and output channels, shared by every client.
//! Stored in `channel-names.json`, next to the preset library, keyed by device serial number.
use std::collections::BTreeMap;

use hyper::{Body, Request, Response};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    device_store::{self, device_key},
    helpers::{parse_body, parse_param, serialize_response},
    Error,
};

const FILE: &str = "channel-names.json";

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

pub async fn get(req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let device = device_key(device_index).await?;
    let names: ChannelNames = device_store::load(FILE, &device.key).await?;
    Ok(serialize_response(&req, names)?)
}

/// Replaces all channel names of a device. Empty names are removed.
pub async fn put(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let device = device_key(device_index).await?;
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
        inputs: clean(requested.inputs, device.inputs, "inputs")?,
        outputs: clean(requested.outputs, device.outputs, "outputs")?,
    };

    let empty = names.inputs.is_empty() && names.outputs.is_empty();
    device_store::store(FILE, &device.key, (!empty).then_some(&names)).await?;
    Ok(serialize_response(&req, names)?)
}
