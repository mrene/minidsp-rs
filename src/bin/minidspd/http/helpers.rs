use super::error::{Error, FormattedError};
use hyper::{body, Body, Request, Response};
use routerify::prelude::*;
use serde::de::DeserializeOwned;
use std::str::FromStr;

pub fn parse_param<T>(req: &Request<Body>, name: &str) -> Result<T, FormattedError>
where
    T: FromStr,
    T::Err: ToString,
{
    let data = req
        .param(name)
        .ok_or_else(|| Error::parameter_missing(name))?;
    Ok(T::from_str(data).map_err(|e| Error::parameter_error(name, e))?)
}

pub async fn parse_body<'de, T: DeserializeOwned>(
    req: &mut Request<Body>,
) -> Result<T, FormattedError> {
    let data = body::to_bytes(req.body_mut())
        .await
        .map_err(|e| Error::ParseError(e.to_string()))?;

    Ok(serde_json::from_slice(data.as_ref()).map_err(|e| Error::ParseError(e.to_string()))?)
}

pub fn serialize_response<T: serde::Serialize>(
    _: &Request<Body>,
    resp: T,
) -> Result<Response<Body>, anyhow::Error> {
    // TODO: Check req content type

    let data = serde_json::to_vec_pretty(&resp)?;

    Ok(Response::builder()
        .header("Content-Type", "application/json")
        .body(Body::from(data))?)
}
