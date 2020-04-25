use crate::transport;
use std::convert::{TryFrom, From, Infallible, Into};


pub struct GetMasterStatusRequest();

impl GetMasterStatusRequest {
    pub fn new() -> Self {
        GetMasterStatusRequest()
    }
}

// impl Into<Vec<u8>> for GetMasterStatusRequest {

impl From<GetMasterStatusRequest> for Vec<u8> {
    fn from(_: GetMasterStatusRequest) -> Vec<u8> {
        vec![0x05, 0xFF, 0xDA, 0x02]
    }
}

pub struct GetMasterStatusResponse {
    pub volume: i16,
    pub mute: bool
}
impl TryFrom<Vec<u8>> for GetMasterStatusResponse {
    type Error = Infallible;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        // TODO: Err if too short
        let volume = -2 * value[3] as i16;
        let mute = value[4] != 0;

        Ok(GetMasterStatusResponse{volume,mute})
    }
}
// impl From<&[u8]> for GetMasterStatusResponse {
//     fn from(value: &[u8]) -> Self {
//         GetMasterStatusResponse::try_from(value).expect("unable to parse response")
//     }
// }