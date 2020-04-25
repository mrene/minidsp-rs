extern crate hidapi;

use crate::commands::{GetMasterStatusRequest, GetMasterStatusResponse};
use std::error::Error;

pub mod transport;
pub mod commands;

pub struct MiniDSP<TTransport>
{
    transport: TTransport
}

impl<TTransport> MiniDSP<TTransport> {
    pub fn new(transport: TTransport) -> Self {
        MiniDSP { transport }
    }
}

impl<Transport: transport::Transport> MiniDSP<Transport> {

    pub fn get_master_status(&mut self) -> Result<(i16, bool), transport::Error<Transport::Error>> {
        let resp: GetMasterStatusResponse = self.transport.execute(GetMasterStatusRequest())?;

        Ok((resp.volume, resp.mute))
    }

}