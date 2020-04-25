use std::convert::{TryFrom, Into, TryInto};
use crate::transport::Error::{MalformedResponse, TransportError};
use std::borrow::Borrow;

mod hid;
pub use hid::HID;

pub enum Error<E> {
    TransportError(E),
    MalformedResponse
}

/*
Basic trait for devices capable of handling minidsp commands
 */
pub trait Transport {
    type Error: std::error::Error;

    fn roundtrip(&self, packet: &[u8]) -> Result<Vec<u8>, Self::Error>;

    fn execute<'a, Request, Response>(&self, request: Request) -> Result<Response, Error<Self::Error>>
    where
        Request: Into<Vec<u8>>,
        Response: TryFrom<Vec<u8>>
    {
        let packet: Vec<u8> = request.into();

        let buf = self.roundtrip(packet.as_ref())
            .map_err(|e| TransportError(e))?;

        let resp = buf.try_into().map_err(|_| MalformedResponse)?;

        Ok(resp)
    }
}
