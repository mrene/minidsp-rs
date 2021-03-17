use crate::{device_manager, App};
use hyper::{Body, Request, Response, Server, StatusCode};
use minidsp::{
    model::{Config, StatusSummary},
    MasterStatus, MiniDSP,
};
use routerify::{Router, RouterService};
use serde::Serialize;
use std::{fmt::Debug, net::SocketAddr, str::FromStr};

mod error;
pub use error::{Error, FormattedError};

mod helpers;
use helpers::{parse_body, parse_param, serialize_response};

#[derive(Clone, Debug, Serialize)]
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

fn get_device<'dsp>(app: &App, index: usize) -> Result<MiniDSP<'dsp>, FormattedError> {
    let devices = app.device_manager.devices();

    // Try to find a device whose serial matches the index passed as an argument
    let serial_match = devices.iter().find(|d| match d.device_info() {
        Some(device_info) => device_info.serial == index as u32,
        None => false,
    });

    if let Some(device) = serial_match {
        return Ok(device.to_minidsp().ok_or(Error::DeviceNotReady)?);
    }

    if index >= devices.len() {
        return Err(FormattedError::from(Error::DeviceIndexOutOfRange {
            actual: devices.len(),
            provided: index,
        }));
    }

    Ok(devices[index].to_minidsp().ok_or(Error::DeviceNotReady)?)
}

/// Gets a list of available devices
async fn get_devices(req: Request<Body>) -> Result<Response<Body>, FormattedError> {
    let app = super::APP.clone();
    let app = app.read().await;

    let devices = app.device_manager.devices();
    let devices: Vec<Device> = devices.iter().map(|d| d.as_ref().into()).collect();

    Ok(serialize_response(&req, devices).map_err(|e| Error::InternalError(e.to_string()))?)
}

/// Retrieves the current master status (current preset, master volume and mute, current input source) for a given device (0-based) index
async fn get_master_status(req: Request<Body>) -> Result<Response<Body>, FormattedError> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;

    let app = super::APP.clone();
    let app = app.read().await;
    let device = get_device(&app, device_index)?;
    let status = StatusSummary::fetch(&device)
        .await
        .map_err(FormattedError::from)?;

    Ok(serialize_response(&req, status).map_err(|e| Error::InternalError(e.to_string()))?)
}

/// Updates the device's master status directly
async fn post_master_status(mut req: Request<Body>) -> Result<Response<Body>, FormattedError> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;

    let app = super::APP.clone();
    let app = app.read().await;
    let device = get_device(&app, device_index)?;

    // Apply the requested, changes, then fetch the master status again to return it
    let status: MasterStatus = parse_body(&mut req).await?;
    status.apply(&device).await.map_err(FormattedError::from)?;

    let status = device
        .get_master_status()
        .await
        .map_err(FormattedError::from)?;

    Ok(serialize_response(&req, status).map_err(|e| Error::InternalError(e.to_string()))?)
}

/// Updates the device's configuration based on the defined elements. Anything set will be changed and anything else will be ignored.
/// If a `master_status` object is passed, and the active configuration is changed, it will be applied before anything else. it is therefore
/// safe to change config and apply other changes to the target config using a single call.
// #[post("/<index>/config", data = "<data>")]
// async fn post_config(index: usize, data: Json<Config>) -> Result<(), Json<FormattedError>> {
async fn post_config(mut req: Request<Body>) -> Result<Response<Body>, FormattedError> {
    let device_index: usize = parse_param(&req, "deviceIndex")?;
    let app = super::APP.clone();
    let app = app.read().await;
    let device = get_device(&app, device_index)?;

    let config: Config = parse_body(&mut req).await?;
    config.apply(&device).await.map_err(FormattedError::from)?;
    Ok(Response::new(Body::default()))
}

// Define an error handler function which will accept the `routerify::Error`
// and the request information and generates an appropriate response.
async fn error_handler(err: routerify::RouteError) -> Response<Body> {
    let error = if let Some(err) = err.downcast_ref::<FormattedError>() {
        serde_json::to_string_pretty(err)
            .unwrap_or_else(|_| "unable to serialize error message".to_string())
    } else {
        format!("Something went wrong: {}", err)
    };

    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(error))
        .unwrap()
}

fn router() -> Router<Body, FormattedError> {
    // Create a router and specify the logger middleware and the handlers.
    // Here, "Middleware::pre" means we're adding a pre middleware which will be executed
    // before any route handlers.
    Router::builder()
        // .middleware(Middleware::pre(logger))
        .get("/devices", get_devices)
        .get("/devices/:deviceIndex", get_master_status)
        .post("/devices/:deviceIndex", post_master_status)
        .post("/devices/:deviceIndex/config", post_config)
        .err_handler(error_handler)
        .build()
        .expect("could not build http router")
}

pub async fn main() -> Result<(), anyhow::Error> {
    let service = RouterService::new(router()).expect("while building router service");

    // The address on which the server will be listening.
    let addr = SocketAddr::from_str("0.0.0.0:8989")?;

    // Create a server by passing the created service to `.serve` method.
    let server = Server::bind(&addr).serve(service);

    println!("App is running on: {}", addr);
    if let Err(err) = server.await {
        eprintln!("Server error: {}", err);
    }

    Ok(())
}
