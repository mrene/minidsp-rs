pub use hid::HID;
use hidapi::HidError;

mod hid;

use failure::Fail;

#[derive(Fail, Debug)]
pub enum MiniDSPError {
    #[fail(display = "An HID error has occurred: {}", _0)]
    HIDError(#[cause] HidError),

    #[fail(display = "A malformed packet was received")]
    MalformedResponse,
}

impl From<HidError> for MiniDSPError {
    fn from(e: HidError) -> Self {
        MiniDSPError::HIDError(e)
    }
}

/// A basic trait for a pdu transport with unary request-response semantics
pub trait Transport: Send {
    fn roundtrip(&mut self, packet: &[u8]) -> Result<Vec<u8>, failure::Error>;
}
