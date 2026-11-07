use std::{fmt::Debug, net::SocketAddr, str::FromStr, sync::Arc, time::Duration};

use anyhow::Context;
use futures::{future::join_all, SinkExt, StreamExt};
use hyper::header::{self, HeaderValue};
use hyper::{body::HttpBody, Body, Request, Response, Server, StatusCode};
use hyper_tungstenite::tungstenite::{self, Message};
use minidsp::Source;
use minidsp::{
    model::{Config, MasterStatus, StatusSummary},
    utils::{ErrInto, OwnedJoinHandle},
    MiniDSP,
};
use routerify::{Middleware, Router, RouterService};
use routerify_query::{query_parser, RequestQueryExt};
use schemars::JsonSchema;
use serde::Serialize;
use tokio_stream::wrappers::IntervalStream;
use websocket::websocket_transport_bridge;

use super::{config::HttpServer, device_manager, App};

mod error;
pub use error::{Error, FormattedError};

mod helpers;
use helpers::{parse_body, parse_param, serialize_response};

mod branding;
mod device_store;
mod gui;
mod links;
mod locks;
mod names;
mod openapi;
mod presets;
mod websocket;

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Device {
    pub url: String,
    pub version: Option<minidsp::DeviceInfo>,
    pub product_name: Option<String>,
}

impl From<&device_manager::Device> for Device {
    fn from(dm: &device_manager::Device) -> Self {
        let version = dm.device_info();
        let device_spec = dm.device_spec();
        let product_name = device_spec.map(|d| d.product_name.to_string());

        Self {
            version,
            product_name,
            url: dm.url.clone(),
        }
    }
}

fn get_device(app: &App, index: usize) -> Result<Arc<device_manager::Device>, Error> {
    let device_manager = app
        .device_manager
        .as_ref()
        .ok_or(Error::ApplicationStillInitializing)?;
    device_manager
        .get_device(index)
        .ok_or(Error::DeviceNotReady)
}

async fn get_device_instance<'dsp>(app: &App, index: usize) -> Result<MiniDSP<'dsp>, Error> {
    let device_manager = app
        .device_manager
        .as_ref()
        .ok_or(Error::ApplicationStillInitializing)?;
    let minidsp = device_manager.get_minidsp(index).await;
    minidsp.ok_or(Error::DeviceNotReady)
}

/// Gets a list of available devices
async fn get_devices(req: Request<Body>) -> Result<Response<Body>, Error> {
    let app = super::APP.get().unwrap();
    let app = app.read().await;

    let devices = app
        .device_manager
        .as_ref()
        .ok_or(Error::ApplicationStillInitializing)?
        .devices();

    let devices: Vec<Device> = devices.iter().map(|d| d.as_ref().into()).collect();

    Ok(serialize_response(&req, devices)?)
}

/// Creates a websocket bridge which forwards raw frames to a device
async fn device_bridge(req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let app = super::APP.get().unwrap();
    let app = app.read().await;
    let device = get_device(&app, device_index)?;
    let hub = device.to_hub().ok_or(Error::DeviceNotReady)?;

    if hyper_tungstenite::is_upgrade_request(&req) {
        let (response, websocket) =
            hyper_tungstenite::upgrade(req, None).context("upgrade failed")?;

        tokio::spawn(websocket_transport_bridge(websocket, hub));

        Ok(response)
    } else {
        Ok(Response::builder()
            .status(405)
            .body(Body::empty())
            .err_into::<anyhow::Error>()?)
    }
}

/// Retrieves the current master status (current preset, master volume and mute, current input source) for a given device (0-based) index
async fn get_master_status(req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;

    let app = super::APP.get().unwrap();
    let app = app.read().await;
    let device = get_device_instance(&app, device_index).await?;
    let mut status = StatusSummary::fetch(&device).await?;
    let query_levels = req.query("levels").cloned();
    let query_poll = req.query("poll").cloned();

    if hyper_tungstenite::is_upgrade_request(&req) {
        let (response, websocket) =
            hyper_tungstenite::upgrade(req, None).context("upgrade failed")?;

        tokio::spawn(async move {
            let mut websocket = websocket.await.context("ws await failed")?;

            // Send the initial status right away, then watch for updates
            websocket
                .send(Message::Text(serde_json::to_string(&status).unwrap()))
                .await?;

            let status_stream = device
                .subscribe_master_status()
                .await?
                .filter_map(|master| async move {
                    let summary = StatusSummary {
                        master: minidsp::model::MasterStatus::from(master),
                        ..Default::default()
                    };

                    let s = serde_json::to_string(&summary).unwrap();
                    Some(Ok(Message::Text(s)))
                })
                .boxed();

            let levels = {
                let device = device.clone();
                if query_levels.is_some() {
                    // Use a single shared device instance in order to avoid multiple level queries from being done simultaneously
                    let levels_device = Arc::new(tokio::sync::Mutex::new(device));
                    IntervalStream::new(tokio::time::interval(Duration::from_millis(250)))
                        .filter_map(move |_| {
                            let device = levels_device.clone();
                            async move {
                                // If we are already querying for levels, skip this interval.
                                let device = device.try_lock().ok()?;

                                let (input_levels, output_levels) =
                                    device.get_input_output_levels().await.ok()?;
                                let summary = StatusSummary {
                                    input_levels,
                                    output_levels,
                                    ..Default::default()
                                };
                                let s = serde_json::to_string(&summary).unwrap();
                                Some(Ok::<_, tungstenite::Error>(Message::Text(s)))
                            }
                        })
                        .boxed()
                } else {
                    futures::stream::empty().boxed()
                }
            };

            let polled_status = {
                if query_poll.is_some() {
                    // Use a single shared device instance in order to avoid multiple level queries from being done simultaneously
                    let polled_status_device = Arc::new(tokio::sync::Mutex::new(device));
                    status.input_levels.clear();
                    status.output_levels.clear();
                    let last_status = Arc::new(std::sync::Mutex::new(status));
                    IntervalStream::new(tokio::time::interval(Duration::from_secs(2)))
                        .filter_map(move |_| {
                            let device = polled_status_device.clone();
                            let last_status = last_status.clone();
                            async move {
                                // If we are already waiting for a response, skip this interval.
                                let device = device.try_lock().ok()?;
                                let status = device.get_master_status().await.ok()?;
                                let summary = StatusSummary {
                                    master: status.into(),
                                    ..Default::default()
                                };

                                {
                                    let mut last_status = last_status.lock().unwrap();
                                    if last_status.eq(&summary) {
                                        return None;
                                    } else {
                                        let mut new_status = summary.clone();
                                        std::mem::swap(&mut *last_status, &mut new_status);
                                    }
                                }
                                let s = serde_json::to_string(&summary).unwrap();
                                Some(Ok::<_, tungstenite::Error>(Message::Text(s)))
                            }
                        })
                        .boxed()
                } else {
                    futures::stream::empty().boxed()
                }
            };

            futures::stream::select_all([status_stream, levels, polled_status])
                .forward(websocket)
                .await?;

            Ok::<(), anyhow::Error>(())
        });

        Ok(response)
    } else {
        Ok(serialize_response(&req, status)?)
    }
}

/// Updates the device's master status directly
async fn post_master_status(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let status: MasterStatus = parse_body(&mut req).await?;
    locks::check(
        device_index,
        status.source.is_some(),
        status.preset.is_some(),
    )
    .await?;

    let app = super::APP.get().unwrap();
    let app = app.read().await;
    let device = get_device_instance(&app, device_index).await?;

    // Apply the requested, changes, then fetch the master status again to return it
    status.apply(&device).await?;

    let status = device.get_master_status().await?;

    Ok(serialize_response(&req, status)?)
}

const VOLUME_INCREMENT: f32 = 0.5;
enum VolumeDirection {
    Up,
    Down,
    ToggleMute,
}

impl FromStr for VolumeDirection {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "up" => Ok(VolumeDirection::Up),
            "down" => Ok(VolumeDirection::Down),
            "toggle_mute" => Ok(VolumeDirection::ToggleMute),
            _ => Err(Error::ParseError {
                reason: "Incorrect value".to_string(),
            }),
        }
    }
}
async fn update_volume(req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let direction: VolumeDirection = parse_param(&req, "direction")?;

    let app = super::APP.get().unwrap();
    let app = app.read().await;
    let device = get_device_instance(&app, device_index).await?;

    let status = device.get_master_status().await?;
    let current_gain = status
        .volume
        .ok_or_else(|| {
            minidsp::MiniDSPError::MalformedResponse("master volume not reported".into())
        })?
        .0;
    let current_mute = status.mute.ok_or_else(|| {
        minidsp::MiniDSPError::MalformedResponse("mute status not reported".into())
    })?;
    match direction {
        VolumeDirection::Down => {
            device
                .set_master_volume(minidsp::Gain(current_gain - VOLUME_INCREMENT))
                .await?;
        }
        VolumeDirection::Up => {
            device
                .set_master_volume(minidsp::Gain(current_gain + VOLUME_INCREMENT))
                .await?;
        }
        VolumeDirection::ToggleMute => {
            device.set_master_mute(!current_mute).await?;
        }
    };
    let status = device.get_master_status().await?;
    Ok(serialize_response(&req, status)?)
}

async fn update_source(req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let source: Source = parse_param(&req, "source")?;
    locks::check(device_index, true, false).await?;

    let app = super::APP.get().unwrap();
    let app = app.read().await;
    let device = get_device_instance(&app, device_index).await?;

    device.set_source(source).await?;
    let status = device.get_master_status().await?;
    Ok(serialize_response(&req, status)?)
}

async fn update_preset(req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let preset: u8 = parse_param(&req, "preset")?;
    locks::check(device_index, false, true).await?;

    let app = super::APP.get().unwrap();
    let app = app.read().await;
    let device = get_device_instance(&app, device_index).await?;

    device.set_config(preset).await?;
    let status = device.get_master_status().await?;
    Ok(serialize_response(&req, status)?)
}
/// Updates the device's configuration based on the defined elements. Anything set will be changed and anything else will be ignored.
/// If a `master_status` object is passed, and the active configuration is changed, it will be applied before anything else. it is therefore
/// safe to change config and apply other changes to the target config using a single call.
async fn post_config(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let config: Config = parse_body(&mut req).await?;
    // Checked before taking the application lock, which the check also reads
    if let Some(master) = &config.master_status {
        locks::check(
            device_index,
            master.source.is_some(),
            master.preset.is_some(),
        )
        .await?;
    }

    let app = super::APP.get().unwrap();
    let app = app.read().await;
    let device = get_device_instance(&app, device_index).await?;
    config.apply(&device).await?;
    Ok(Response::new(Body::default()))
}

async fn schema_fn<T: JsonSchema>(req: Request<Body>) -> Result<Response<Body>, Error> {
    use schemars::schema_for;
    Ok(serialize_response(&req, schema_for!(T))?)
}

async fn redoc(_: Request<Body>) -> Result<Response<Body>, Error> {
    Ok(Response::new(Body::from(include_str!(
        "openapi/redoc.html"
    ))))
}

// Define an error handler function which will accept the `routerify::Error`
// and the request information and generates an appropriate response.
async fn error_handler(err: routerify::RouteError) -> Response<Body> {
    let (status, error) = if let Some(err) = err.downcast_ref::<Error>() {
        // Problems with the request are the client's to fix; the rest are on our side
        let status = match err {
            Error::ParseError { .. }
            | Error::ParameterError { .. }
            | Error::DeviceIndexOutOfRange { .. } => StatusCode::BAD_REQUEST,
            Error::ApplicationStillInitializing | Error::DeviceNotReady => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            Error::Locked { .. } => StatusCode::LOCKED,
            Error::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let err: FormattedError = err.clone().into();
        let body = serde_json::to_string_pretty(&err).unwrap_or_else(|e| {
            format!("the error: '{err:?}' couldn't be serialized as json: {e:?}")
        });
        (status, body)
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {err}"),
        )
    };

    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(error))
        .unwrap()
}

fn router(server: Option<HttpServer>) -> Router<Body, Error> {
    // Create a router and specify the logger middleware and the handlers.
    // Here, "Middleware::pre" means we're adding a pre middleware which will be executed
    // before any route handlers.
    let mut builder = Router::builder()
        .middleware(query_parser())
        // .middleware(Middleware::pre(logger))
        .get("/openapi.json", |req| async move {
            Ok(serialize_response(&req, openapi::schema())?)
        })
        .get("/devices", get_devices)
        .get("/devices/get.schema", schema_fn::<Vec<Device>>)
        .get("/devices/:deviceIndex", get_master_status)
        .post("/devices/:deviceIndex", post_master_status)
        .post("/devices/:deviceIndex/volume/:direction", update_volume)
        .post("/devices/:deviceIndex/source/:source", update_source)
        .post("/devices/:deviceIndex/preset/:preset", update_preset)
        .get(
            "/devices/:deviceIndex/get.schema",
            schema_fn::<StatusSummary>,
        )
        .get(
            "/devices/:deviceIndex/post.schema",
            schema_fn::<MasterStatus>,
        )
        .post("/devices/:deviceIndex/config", post_config)
        .get("/devices/:deviceIndex/layout", gui::get_layout)
        .get("/devices/:deviceIndex/names", names::get)
        .put("/devices/:deviceIndex/names", names::put)
        .get("/devices/:deviceIndex/links", links::get)
        .put("/devices/:deviceIndex/links", links::put)
        .get("/devices/:deviceIndex/locks", locks::get)
        .put("/devices/:deviceIndex/locks", locks::put)
        .post(
            "/devices/:deviceIndex/presets/:presetId/apply",
            presets::apply,
        )
        .get("/branding", branding::get)
        .put("/branding", branding::put)
        .get("/presets", presets::list)
        .post("/presets", presets::create)
        .get("/deleted-presets", presets::list_deleted)
        .get("/presets/get.schema", schema_fn::<presets::Preset>)
        .get("/presets/:presetId", presets::get)
        .put("/presets/:presetId", presets::update)
        .delete("/presets/:presetId", presets::delete)
        .post("/presets/:presetId/rename", presets::rename)
        .get("/presets/:presetId/history", presets::history)
        .post("/presets/:presetId/rollback", presets::rollback)
        .post("/presets/:presetId/undo", presets::undo)
        .post("/presets/:presetId/restore", presets::restore)
        .get(
            "/devices/:deviceIndex/layout/get.schema",
            schema_fn::<gui::DeviceLayout>,
        )
        .get(
            "/devices/:deviceIndex/config/post.schema",
            schema_fn::<Config>,
        )
        .get("/api", redoc)
        .any_method("/devices/:deviceIndex/ws", device_bridge)
        .err_handler(error_handler);

    if let Some(server) = server {
        if let Some(allowed_origins) = server.allowed_origins {
            builder = builder.middleware(enable_cors_all(allowed_origins.into()));
        }

        // Registered last so that API routes always take precedence over static files
        if let Some(static_dir) = server.static_dir {
            let root = std::path::PathBuf::from(static_dir);
            log::info!("Serving web interface from {}", root.display());
            builder = builder.get("/*", move |req| gui::serve_static(root.clone(), req));
        }
    }

    builder.build().expect("could not build http router")
}

pub async fn tcp_main(server: HttpServer) -> Result<(), anyhow::Error> {
    let bind_address = server
        .bind_address
        .as_deref()
        .unwrap_or("0.0.0.0:5380")
        .to_owned();

    let rt = router(Some(server));
    let service = RouterService::new(rt).expect("while building router service");

    // The address on which the server will be listening.
    let addr = SocketAddr::from_str(&bind_address)?;

    // Create a server by passing the created service to `.serve` method.
    let server = Server::try_bind(&addr)?.serve(service);

    println!("App is running on: {addr}");
    if let Err(err) = server.await {
        eprintln!("TCP Server error: {err:?}");
        return Err(err.into());
    }

    Ok(())
}

#[cfg(target_family = "unix")]
pub async fn unix_main() -> Result<(), anyhow::Error> {
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    use hyperlocal::UnixServerExt;
    use routerify_unixsocket::UnixRouterService;

    let service = UnixRouterService::new(router(None)).expect("while building router service");

    let path = Path::new("/tmp/minidsp.sock");
    if path.exists() {
        std::fs::remove_file(path).context("deleting existing unix socket file")?;
    }

    // Create a server by passing the created service to `.serve` method.
    let server = Server::bind_unix(path)
        .context("couldn't bind unix socket")?
        .serve(service);

    // Change the socket's permissions so other users can connect
    let m = fs::metadata(path).context("cannot get unix socket file metadata")?;
    let mut perms = m.permissions();
    perms.set_mode(0o766);
    fs::set_permissions(path, perms).context("couldn't set unix socket file permissions")?;

    println!("App is listening on: {}", path.to_string_lossy());
    if let Err(err) = server.await {
        eprintln!("Unix Server error: {err:?}");
    }

    Ok(())
}

pub fn enable_cors_all<B, E>(allowed_origins: Arc<str>) -> Middleware<B, E>
where
    B: HttpBody + Send + Sync + Unpin + 'static,
    E: std::error::Error + Send + Sync + Unpin + 'static,
{
    let handler = Box::new(move |mut res: Response<B>| {
        let allowed_origins = allowed_origins.clone();
        async move {
            let headers = res.headers_mut();
            let value =
                HeaderValue::from_str(&allowed_origins).expect("cors header value was invalid");

            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value.clone());
            headers.insert(header::ACCESS_CONTROL_ALLOW_METHODS, value.clone());
            headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, value.clone());
            headers.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, value);

            Ok(res)
        }
    });

    Middleware::post(handler)
}

pub async fn main(cfg: Option<HttpServer>) -> Result<(), anyhow::Error> {
    let mut futs: Vec<OwnedJoinHandle<Result<(), anyhow::Error>>> = Vec::with_capacity(2);

    if let Some(server) = cfg {
        let server = server.clone();

        futs.push(
            tokio::spawn(async move {
                if let Err(e) = tcp_main(server).await {
                    eprintln!("HTTP/TCP listener error: {}", &e);
                    return Err(e);
                }
                Ok(())
            })
            .into(),
        );
    }

    #[cfg(target_family = "unix")]
    futs.push(
        tokio::spawn(async {
            if let Err(e) = unix_main().await {
                eprintln!("HTTP/Unix listener error: {}", &e);
                return Err(e);
            }
            Ok(())
        })
        .into(),
    );

    join_all(futs).await;

    Ok(())
}
