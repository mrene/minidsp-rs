//! Support routes for the web interface: a description of each device's channels and features,
//! and serving the interface's static files.
use std::path::{Component, Path, PathBuf};

use hyper::{Body, Request, Response, StatusCode};
use minidsp::Source;
use schemars::JsonSchema;
use serde::Serialize;

use super::{get_device, helpers::parse_param, helpers::serialize_response, Error};

/// Describes what can be controlled on a device, so that a user interface can render it
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct DeviceLayout {
    pub product_name: String,
    /// Input sources that can be selected on this device
    pub sources: Vec<Source>,
    /// Internal sampling rate in Hz, which determines the maximum output delay
    pub sampling_rate: u32,
    /// Maximum output delay, in milliseconds
    pub max_delay_ms: f64,
    pub inputs: Vec<InputLayout>,
    pub outputs: Vec<OutputLayout>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct InputLayout {
    /// Whether the input can be muted
    pub has_mute: bool,
    /// Whether the input gain can be set
    pub has_gain: bool,
    /// Whether this input reports a level. Levels returned by the status endpoint only include
    /// channels that have a meter, in channel order.
    pub has_meter: bool,
    /// Number of parametric equalizers
    pub peq_count: usize,
    /// Number of outputs this input can be routed to
    pub routing_count: usize,
    /// Whether the routing matrix entries of this input have a gain setting
    pub routing_has_gain: bool,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct OutputLayout {
    /// Whether the output gain can be set
    pub has_gain: bool,
    /// Whether this output reports a level. Levels returned by the status endpoint only include
    /// channels that have a meter, in channel order.
    pub has_meter: bool,
    /// Whether the output delay can be set
    pub has_delay: bool,
    /// Number of parametric equalizers
    pub peq_count: usize,
    pub has_crossover: bool,
    /// Number of crossover groups, each holding 4 cascaded biquads
    pub crossover_groups: usize,
    pub has_compressor: bool,
    pub has_fir: bool,
    /// Maximum number of FIR taps, 0 if there is no FIR filter
    pub fir_max_taps: u16,
}

impl DeviceLayout {
    pub fn new(spec: &minidsp::device::Device, info: Option<&minidsp::DeviceInfo>) -> Self {
        let sources = match info {
            Some(info) => Source::mapping(info).iter().map(|(s, _)| *s).collect(),
            None => spec.sources.to_vec(),
        };

        // The device accepts at most 8000 samples of delay (see `Output::set_delay`)
        let max_delay_ms = if spec.internal_sampling_rate > 0 {
            8000. * 1000. / spec.internal_sampling_rate as f64
        } else {
            0.
        };

        Self {
            product_name: spec.product_name.to_string(),
            sources,
            sampling_rate: spec.internal_sampling_rate,
            max_delay_ms,
            inputs: spec
                .inputs
                .iter()
                .map(|i| InputLayout {
                    has_mute: i.gate.is_some(),
                    has_gain: i.gate.as_ref().is_some_and(|g| g.gain.is_some()),
                    has_meter: i.meter.is_some(),
                    peq_count: i.peq.len(),
                    routing_count: i.routing.len(),
                    routing_has_gain: !i.routing.is_empty()
                        && i.routing.iter().all(|r| r.gain.is_some()),
                })
                .collect(),
            outputs: spec
                .outputs
                .iter()
                .map(|o| OutputLayout {
                    has_gain: o.gate.gain.is_some(),
                    has_meter: o.meter.is_some(),
                    has_delay: o.delay_addr.is_some(),
                    peq_count: o.peq.len(),
                    has_crossover: o.xover.is_some(),
                    crossover_groups: o.xover.as_ref().map_or(0, |x| x.peqs.len()),
                    has_compressor: o.compressor.is_some(),
                    has_fir: o.fir.is_some(),
                    fir_max_taps: o.fir.as_ref().map_or(0, |f| f.max_coefficients),
                })
                .collect(),
        }
    }
}

/// Describes the channels and features of a given device (0-based index)
pub async fn get_layout(req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let app = super::super::APP.get().unwrap();
    let app = app.read().await;
    let device = get_device(&app, device_index)?;
    let spec = device.device_spec().ok_or(Error::DeviceNotReady)?;
    let info = device.device_info();

    Ok(serialize_response(
        &req,
        DeviceLayout::new(spec, info.as_ref()),
    )?)
}

/// Maps a request path onto a file inside `root`, refusing anything that could escape it
fn resolve_path(root: &Path, request_path: &str) -> Option<PathBuf> {
    let relative = request_path.trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };

    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => path.push(part),
            // `..`, absolute paths and prefixes could leave the static directory
            _ => return None,
        }
    }
    Some(path)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Serves a file from the web interface's build directory
pub async fn serve_static(root: PathBuf, req: Request<Body>) -> Result<Response<Body>, Error> {
    let not_found = || {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("not found"))
            .map_err(|e| Error::from(std::sync::Arc::new(anyhow::Error::from(e))))
    };

    let Some(path) = resolve_path(&root, req.uri().path()) else {
        return not_found();
    };

    match tokio::fs::read(&path).await {
        Ok(data) => {
            // Hashed build assets never change, the entry page must always be revalidated
            let cache = if path.starts_with(root.join("assets")) {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            Response::builder()
                .header("Content-Type", content_type(&path))
                .header("Cache-Control", cache)
                .body(Body::from(data))
                .map_err(|e| Error::from(std::sync::Arc::new(anyhow::Error::from(e))))
        }
        Err(_) => not_found(),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn resolve_path_stays_inside_root() {
        let root = Path::new("/srv/gui");
        assert_eq!(
            resolve_path(root, "/"),
            Some(PathBuf::from("/srv/gui/index.html"))
        );
        assert_eq!(
            resolve_path(root, "/assets/app.js"),
            Some(PathBuf::from("/srv/gui/assets/app.js"))
        );
        assert_eq!(resolve_path(root, "/../etc/passwd"), None);
        assert_eq!(resolve_path(root, "/assets/../../etc/passwd"), None);
    }
}
