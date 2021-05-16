use std::{fmt::Debug, net::SocketAddr, str::FromStr, sync::Arc, time::Duration};

use anyhow::Context;
use futures::{future::join_all, SinkExt, StreamExt};
use hyper::{Body, Request, Response, Server, StatusCode};
use minidsp::{
    model::{Config, MasterStatus, StatusSummary},
    utils::{ErrInto, OwnedJoinHandle},
    MiniDSP,
};
use routerify::{Router, RouterService};
use routerify_query::{query_parser, RequestQueryExt};
use schemars::JsonSchema;
use serde::Serialize;
use tokio_stream::wrappers::IntervalStream;
use tungstenite::Message;
use websocket::websocket_transport_bridge;

use super::{config::HttpServer, device_manager, App};

mod error;
pub use error::{Error, FormattedError};

mod helpers;
use helpers::{parse_body, parse_param, serialize_response};

mod openapi;
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
    let devices = app
        .device_manager
        .as_ref()
        .ok_or(Error::ApplicationStillInitializing)?
        .devices();

    // Try to find a device whose serial matches the index passed as an argument
    let serial_match = devices.iter().find(|d| match d.device_info() {
        Some(device_info) => device_info.serial == index as u32,
        None => false,
    });

    if let Some(device) = serial_match {
        return Ok(device.clone());
    }

    if index >= devices.len() {
        return Err(Error::DeviceIndexOutOfRange {
            actual: devices.len(),
            provided: index,
        });
    }

    Ok(devices[index].clone())
}

fn get_device_instance<'dsp>(app: &App, index: usize) -> Result<MiniDSP<'dsp>, Error> {
    get_device(app, index)?
        .to_minidsp()
        .ok_or(Error::DeviceNotReady)
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
    let device = get_device_instance(&app, device_index)?;
    let status = StatusSummary::fetch(&device).await?;
    let query_levels = req.query("levels").map(Clone::clone);

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

            futures::stream::select_all(std::array::IntoIter::new([status_stream, levels]))
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

    let app = super::APP.get().unwrap();
    let app = app.read().await;
    let device = get_device_instance(&app, device_index)?;

    // Apply the requested, changes, then fetch the master status again to return it
    let status: MasterStatus = parse_body(&mut req).await?;
    status.apply(&device).await?;

    let status = device.get_master_status().await?;

    Ok(serialize_response(&req, status)?)
}

/// Updates the device's configuration based on the defined elements. Anything set will be changed and anything else will be ignored.
/// If a `master_status` object is passed, and the active configuration is changed, it will be applied before anything else. it is therefore
/// safe to change config and apply other changes to the target config using a single call.
async fn post_config(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let app = super::APP.get().unwrap();
    let app = app.read().await;
    let device = get_device_instance(&app, device_index)?;

    let config: Config = parse_body(&mut req).await?;
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
    let error = if let Some(err) = err.downcast_ref::<Error>() {
        let err: FormattedError = err.clone().into();
        serde_json::to_string_pretty(&err).unwrap_or_else(|e| {
            format!(
                "the error: '{:?}' couldn't be serialized as json: {:?}",
                err, e
            )
        })
    } else {
        format!("Something went wrong: {}", err)
    };

    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(error))
        .unwrap()
}

fn router() -> Router<Body, Error> {
    // Create a router and specify the logger middleware and the handlers.
    // Here, "Middleware::pre" means we're adding a pre middleware which will be executed
    // before any route handlers.
    Router::builder()
        .middleware(query_parser())
        // .middleware(Middleware::pre(logger))
        .get("/openapi.json", |req| async move {
            Ok(serialize_response(&req, openapi::schema())?)
        })
        .get("/devices", get_devices)
        .get("/devices/get.schema", schema_fn::<Vec<Device>>)
        .get("/devices/:deviceIndex", get_master_status)
        .post("/devices/:deviceIndex", post_master_status)
        .get(
            "/devices/:deviceIndex/get.schema",
            schema_fn::<StatusSummary>,
        )
        .get(
            "/devices/:deviceIndex/post.schema",
            schema_fn::<MasterStatus>,
        )
        .post("/devices/:deviceIndex/config", post_config)
        .get(
            "/devices/:deviceIndex/config/post.schema",
            schema_fn::<Config>,
        )
        .get("/api", redoc)
        .any_method("/devices/:deviceIndex/ws", device_bridge)
        .err_handler(error_handler)
        .build()
        .expect("could not build http router")
}

pub async fn tcp_main(bind_address: String) -> Result<(), anyhow::Error> {
    let rt = router();
    let service = RouterService::new(rt).expect("while building router service");

    // The address on which the server will be listening.
    let addr = SocketAddr::from_str(&bind_address)?;

    // Create a server by passing the created service to `.serve` method.
    let server = Server::try_bind(&addr)?.serve(service);

    println!("App is running on: {}", addr);
    if let Err(err) = server.await {
        eprintln!("TCP Server error: {:?}", err);
        return Err(err.into());
    }

    Ok(())
}

#[cfg(target_family = "unix")]
pub async fn unix_main() -> Result<(), anyhow::Error> {
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    use hyperlocal::UnixServerExt;
    use routerify_unixsocket::UnixRouterService;

    let service = UnixRouterService::new(router()).expect("while building router service");

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
        eprintln!("Unix Server error: {:?}", err);
    }

    Ok(())
}

pub async fn main(cfg: Option<HttpServer>) -> Result<(), anyhow::Error> {
    let mut futs: Vec<OwnedJoinHandle<Result<(), anyhow::Error>>> = Vec::with_capacity(2);

    if let Some(server) = cfg {
        let bind_address = server
            .bind_address
            .as_deref()
            .unwrap_or("0.0.0.0:5380")
            .to_owned();

        futs.push(
            tokio::spawn(async {
                if let Err(e) = tcp_main(bind_address).await {
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

    join_all(futs.into_iter()).await;

    Ok(())
}
