use minidsp::MiniDSPError;
use thiserror::Error;

#[derive(Clone, Debug, serde::Serialize, Error)]
#[serde(tag = "type")]
pub enum Error {
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

    #[error("an internal error occurred: {0}")]
    InternalError(String),
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

impl std::fmt::Display for FormattedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Error as std::fmt::Display>::fmt(&self.error, f)
    }
}

impl std::error::Error for FormattedError {}

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
