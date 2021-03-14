use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
};

use crate::{device_manager, App};
use minidsp::{
    model::{Config, StatusSummary},
    transport::MiniDSPError,
    utils::ErrInto,
    MasterStatus, MiniDSP,
};
use rocket::{get, post, routes};
use rocket_contrib::json::Json;
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Error)]
#[serde(tag = "type")]
pub enum Error {
    #[error(
        "device index was out of range. provided value {provided} was not in range [0, {actual})"
    )]
    DeviceIndexOutOfRange { provided: usize, actual: usize },

    #[error("the specified device is not ready to accept requests")]
    DeviceNotReady,

    #[error("an internal error occurred: {0}")]
    InternalError(String),
}

impl From<MiniDSPError> for Error {
    fn from(e: MiniDSPError) -> Self {
        // TODO: Once errors are cleaner, map this correctly
        Self::InternalError(e.to_string())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FormattedError {
    message: String,
    error: Error,
}

impl From<MiniDSPError> for FormattedError {
    fn from(e: MiniDSPError) -> Self {
        e.into()
    }
}

impl From<Error> for FormattedError {
    fn from(error: Error) -> Self {
        Self {
            message: error.to_string(),
            error,
        }
    }
}

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
#[get("/")]
async fn devices() -> Json<Vec<Device>> {
    let app = super::APP.clone();
    let app = app.read().await;

    let devices = app.device_manager.devices();
    devices
        .iter()
        .map(|d| d.as_ref().into())
        .collect::<Vec<_>>()
        .into()
}

/// Retrieves the current master status (current preset, master volume and mute, current input source) for a given device (0-based) index
#[get("/<index>")]
async fn master_status(index: usize) -> Result<Json<StatusSummary>, Json<FormattedError>> {
    let app = super::APP.clone();
    let app = app.read().await;
    let device = get_device(&app, index)?;
    Ok(StatusSummary::fetch(&device)
        .await
        .map_err(FormattedError::from)?
        .into())
}

/// Updates the device's master status directly
#[post("/<index>", data = "<data>")]
async fn post_master_status(
    index: usize,
    data: Json<MasterStatus>,
) -> Result<Json<MasterStatus>, Json<FormattedError>> {
    let app = super::APP.clone();
    let app = app.read().await;
    let device = get_device(&app, index)?;

    // Apply the requested, changes, then fetch the master status again to return it
    let status = data.into_inner();
    status.apply(&device).await.map_err(FormattedError::from)?;

    Ok(device
        .get_master_status()
        .await
        .map_err(FormattedError::from)?
        .into())
}

/// Updates the device's configuration based on the defined elements. Anything set will be changed and anything else will be ignored.
/// If a `master_status` object is passed, and the active configuration is changed, it will be applied before anything else. it is therefore
/// safe to change config and apply other changes to the target config using a single call.
#[post("/<index>/config", data = "<data>")]
async fn post_config(index: usize, data: Json<Config>) -> Result<(), Json<FormattedError>> {
    let app = super::APP.clone();
    let app = app.read().await;
    let device = get_device(&app, index)?;

    // Apply the requested, changes, then fetch the master status again to return it
    let config = data.into_inner();
    config.apply(&device).await.map_err(FormattedError::from)?;
    Ok(())
}

pub async fn main() -> Result<(), anyhow::Error> {
    let app = super::APP.clone();
    let app = app.read().await;

    let mut config = rocket::config::Config {
        address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        port: 5380,
        ..Default::default()
    };

    if let Some(ref http) = app.opts.http {
        let addr = SocketAddr::from_str(http)?;
        config.address = addr.ip();
        config.port = addr.port();
    }

    let ship = rocket::custom(config).mount(
        "/devices",
        routes![devices, master_status, post_master_status, post_config],
    );
    let result = ship.launch().await;
    match &result {
        Ok(_) => {
            println!("HTTP server terminated");
            std::process::exit(0);
        }
        Err(e) => eprintln!("HTTP server error: {}", &e),
    }

    result.err_into()
}
