//! Linked channels: groups of inputs or outputs (e.g. left and right woofer) that share
//! settings. Clients apply a change to one member to every member of its group.
//! Stored in `channel-links.json`, next to the preset library, keyed by device serial number.
use std::collections::BTreeSet;

use hyper::{Body, Request, Response};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    device_store::{self, device_key},
    helpers::{parse_body, parse_param, serialize_response},
    Error,
};

const FILE: &str = "channel-links.json";

/// Settings a link group can share
pub const PARAMS: [&str; 8] = [
    "peq",
    "crossover",
    "compressor",
    "fir",
    "gain",
    "mute",
    "delay",
    "invert",
];

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChannelKind {
    In,
    Out,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LinkGroup {
    pub kind: ChannelKind,
    /// 0-based channel indexes, at least two
    pub channels: Vec<usize>,
    /// Which settings are shared, from: peq, crossover, compressor, fir, gain, mute, delay, invert
    pub params: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ChannelLinks {
    #[serde(default)]
    pub groups: Vec<LinkGroup>,
}

pub async fn get(req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let device = device_key(device_index).await?;
    let links: ChannelLinks = device_store::load(FILE, &device.key).await?;
    Ok(serialize_response(&req, links)?)
}

fn validate(links: ChannelLinks, inputs: usize, outputs: usize) -> Result<ChannelLinks, Error> {
    let bad = |reason: String| Error::ParseError { reason };
    let mut used_in = BTreeSet::new();
    let mut used_out = BTreeSet::new();
    let mut groups = Vec::new();

    for mut group in links.groups {
        let (count, used, kind) = match group.kind {
            ChannelKind::In => (inputs, &mut used_in, "IN"),
            ChannelKind::Out => (outputs, &mut used_out, "OUT"),
        };
        group.channels.sort_unstable();
        group.channels.dedup();
        if group.channels.len() < 2 {
            return Err(bad("a link needs at least two channels".into()));
        }
        for &ch in &group.channels {
            if ch >= count {
                return Err(bad(format!("this device has no {kind} {}", ch + 1)));
            }
            if !used.insert(ch) {
                return Err(bad(format!("{kind} {} is in more than one link", ch + 1)));
            }
        }
        group.params.sort();
        group.params.dedup();
        if let Some(p) = group.params.iter().find(|p| !PARAMS.contains(&p.as_str())) {
            return Err(bad(format!("unknown linked setting '{p}'")));
        }
        if group.params.is_empty() {
            return Err(bad("a link needs at least one shared setting".into()));
        }
        groups.push(group);
    }
    Ok(ChannelLinks { groups })
}

/// Replaces all link groups of a device
pub async fn put(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let device = device_key(device_index).await?;
    let links = validate(parse_body(&mut req).await?, device.inputs, device.outputs)?;
    let empty = links.groups.is_empty();
    device_store::store(FILE, &device.key, (!empty).then_some(&links)).await?;
    Ok(serialize_response(&req, links)?)
}

#[cfg(test)]
mod test {
    use super::*;

    fn group(kind: ChannelKind, channels: &[usize], params: &[&str]) -> LinkGroup {
        LinkGroup {
            kind,
            channels: channels.to_vec(),
            params: params.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn validates_link_groups() {
        let links = |groups| ChannelLinks { groups };
        let ok = validate(
            links(vec![
                group(ChannelKind::Out, &[1, 0, 1], &["peq", "gain"]),
                group(ChannelKind::In, &[0, 1], &["peq"]),
            ]),
            2,
            4,
        )
        .unwrap();
        assert_eq!(ok.groups[0].channels, vec![0, 1]);

        // A channel can't be in two groups, but an input and an output with the same index can
        assert!(validate(
            links(vec![
                group(ChannelKind::Out, &[0, 1], &["peq"]),
                group(ChannelKind::Out, &[1, 2], &["peq"]),
            ]),
            2,
            4
        )
        .is_err());
        assert!(validate(links(vec![group(ChannelKind::Out, &[0], &["peq"])]), 2, 4).is_err());
        assert!(validate(
            links(vec![group(ChannelKind::Out, &[0, 4], &["peq"])]),
            2,
            4
        )
        .is_err());
        assert!(validate(
            links(vec![group(ChannelKind::In, &[0, 1], &["volume"])]),
            2,
            4
        )
        .is_err());
        assert!(validate(links(vec![group(ChannelKind::In, &[0, 1], &[])]), 2, 4).is_err());
    }
}
