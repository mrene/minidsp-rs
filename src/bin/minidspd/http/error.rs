use minidsp::MiniDSPError;
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, serde::Serialize, Error)]
#[serde(tag = "type")]
pub enum Error {
    #[error("the application is still being initialized")]
    ApplicationStillInitializing,

    #[error(
        "device index was out of range. provided value {provided} was not in range [0, {actual})"
    )]
    DeviceIndexOutOfRange { provided: usize, actual: usize },

    #[error("couldn't parse parameter named {name}: {error}")]
    ParameterError { name: String, error: String },

    #[error("the request could not be parsed")]
    ParseError(String),

    #[error("the specified device is not ready to accept requests")]
    DeviceNotReady,

    #[error(transparent)]
    #[serde(serialize_with = "ser_to_string")]
    InternalError(#[from] Arc<anyhow::Error>),
}

fn ser_to_string<S, T>(t: &T, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    T: ToString,
{
    s.serialize_str(t.to_string().as_str())
}

impl Error {
    pub fn parameter_error<E: ToString>(name: &str, error: E) -> Self {
        Error::ParameterError {
            name: name.to_string(),
            error: error.to_string(),
        }
    }

    pub fn parameter_missing(name: &str) -> Self {
        Error::ParameterError {
            name: name.to_string(),
            error: "parameter is missing".to_string(),
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Self::from(Arc::new(e))
    }
}

impl From<MiniDSPError> for Error {
    fn from(e: MiniDSPError) -> Self {
        // TODO: Once errors are cleaner, map this correctly
        Self::InternalError(Arc::new(e.into()))
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct FormattedError {
    message: String,
    error: Error,
}

impl std::fmt::Display for FormattedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Error as std::fmt::Display>::fmt(&self.error, f)
    }
}

impl std::error::Error for FormattedError {}

impl From<Error> for FormattedError {
    fn from(error: Error) -> Self {
        Self {
            message: error.to_string(),
            error,
        }
    }
}
