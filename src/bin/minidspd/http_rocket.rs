use std::sync::Arc;

use crate::{device_manager, App};
use minidsp::{MasterStatus, MiniDSP, model::Config, transport::MiniDSPError, utils::ErrInto};
use rocket::{catchers, get, post, routes};
use rocket_contrib::json::{Json, JsonValue};
use thiserror::Error;

#[derive(serde::Serialize, Clone, Debug, Error)]
pub enum Error {
    #[error(
        "device index was out of range. provided value {provided} was not in range [0, {actual})"
    )]
    DeviceIndexOutOfRange { provided: usize, actual: usize },

    #[error("an internal error occurred: {0}")]
    InternalError(String),
}

impl From<MiniDSPError> for Error {
    fn from(e: MiniDSPError) -> Self {
        // TODO: Once errors are cleaner, map this correctly
        Self::InternalError(e.to_string())
    }
}

#[derive(Clone, Debug, serde::Serialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Device {
    pub url: String,
}

impl From<&device_manager::Device> for Device {
    fn from(dm: &device_manager::Device) -> Self {
        Self {
            url: dm.url.clone(),
        }
    }
}

fn get_device<'dsp>(app: &App, index: usize) -> Result<MiniDSP<'dsp>, FormattedError> {
    let devices = app.device_manager.devices();
    if index >= devices.len() {
        return Err(FormattedError::from(Error::DeviceIndexOutOfRange {
            actual: devices.len(),
            provided: index,
        }));
    }

    Ok(devices[index].to_minidsp())
}

#[get("/")]
async fn devices() -> Json<Vec<Device>> {
    let app = super::APP.clone();
    let app = app.read().await;
    Json(
        app.device_manager
            .devices()
            .into_iter()
            .map(|d| d.as_ref().into())
            .collect::<Vec<_>>()
            .into(),
    )
}
#[get("/<index>")]
async fn master_status(index: usize) -> Result<Json<MasterStatus>, Json<FormattedError>> {
    let app = super::APP.clone();
    let app = app.read().await;
    let device = get_device(&app, index)?;

    Ok(Json(
        device
            .get_master_status()
            .await
            .map_err(FormattedError::from)?,
    ))
}

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

    Ok(Json(
        device
            .get_master_status()
            .await
            .map_err(FormattedError::from)?,
    ))
}

#[post("/<index>/config", data = "<data>")]
async fn post_config(
    index: usize,
    data: Json<Config>,
) -> Result<(), Json<FormattedError>> {
    let app = super::APP.clone();
    let app = app.read().await;
    let device = get_device(&app, index)?;

    // Apply the requested, changes, then fetch the master status again to return it
    let config = data.into_inner();
    config.apply(&device).await.map_err(FormattedError::from)?;
    Ok(())
}

pub async fn main() {
    let ship = rocket::ignite().mount(
        "/devices",
        routes![devices, master_status, post_master_status, post_config],
    );

    let result = ship.launch().await;
    match result {
        Ok(_) => {
            println!("HTTP server terminated");
            std::process::exit(0);
        }
        Err(e) => eprintln!("HTTP server error: {}", &e),
    }
}
