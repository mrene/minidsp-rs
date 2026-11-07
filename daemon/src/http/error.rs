use std::sync::Arc;

use minidsp::MiniDSPError;
use thiserror::Error;

#[derive(Clone, Debug, serde::Serialize, Error, schemars::JsonSchema)]
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
    ParseError { reason: String },

    #[error("the specified device is not ready to accept requests")]
    DeviceNotReady,

    #[error(transparent)]
    #[serde(serialize_with = "ser_details")]
    #[schemars(with = "InternalErrorDetails")]
    InternalError(#[from] Arc<anyhow::Error>),
}

/// How [`Error::InternalError`] is serialized. Internally tagged enums can't hold a bare string,
/// so the error is wrapped in an object: `{"type": "InternalError", "details": "..."}`
#[derive(serde::Serialize, schemars::JsonSchema)]
struct InternalErrorDetails {
    /// The error, including its causes
    details: String,
}

fn ser_details<S>(error: &Arc<anyhow::Error>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serde::Serialize::serialize(
        &InternalErrorDetails {
            details: format!("{error:#}"),
        },
        s,
    )
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

#[derive(Clone, Debug, serde::Serialize, schemars::JsonSchema)]
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
        // Include the chain of causes, the top-level message alone is often just "Internal error"
        let message = match &error {
            Error::InternalError(e) => format!("{e:#}"),
            other => other.to_string(),
        };
        Self { message, error }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn internal_errors_serialize_with_their_causes() {
        let error: Error = anyhow::anyhow!("delay out of range")
            .context("Internal error")
            .into();
        let json = serde_json::to_value(FormattedError::from(error)).unwrap();
        assert_eq!(json["message"], "Internal error: delay out of range");
        assert_eq!(json["error"]["type"], "InternalError");
        assert_eq!(
            json["error"]["details"],
            "Internal error: delay out of range"
        );
    }
}
