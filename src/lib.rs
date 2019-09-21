pub mod bindings;

use crate::bindings::{
    cec_command, cec_datapacket, cec_device_type, cec_device_type_list, cec_keypress,
    cec_logical_address, cec_opcode, libcec_clear_configuration, libcec_close,
    libcec_configuration, libcec_connection_t, libcec_destroy, libcec_enable_callbacks,
    libcec_initialise, libcec_open, libcec_transmit, libcec_version, ICECCallbacks,
};
use std::ffi::CStr;
use std::{mem, result};

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    LibInitFailed,
    NoAdapterFound,
    AdapterOpenFailed,
    CallbackRegistrationFailed,
    TransmitFailed,
}

impl cec_datapacket {
    pub fn empty() -> cec_datapacket {
        cec_datapacket {
            data: [0; 64], // FIXME: how to avoid hardcoding default
            size: 0,
        }
    }
}
impl cec_command {
    pub fn set_system_audio_mode(set_mode: bool, destination: cec_logical_address) -> cec_command {
        cec_command {
            initiator: cec_logical_address::CECDEVICE_UNKNOWN,
            destination,
            ack: 1,
            eom: 1,
            opcode: cec_opcode::SET_SYSTEM_AUDIO_MODE,
            parameters: cec_datapacket::empty(),
            opcode_set: if set_mode { 1 } else { 0 },
            transmit_timeout: 1000,
        }
    }
}

pub struct CecConnection {
    conn: libcec_connection_t,
    config: libcec_configuration,
}

impl CecConnection {
    pub fn new(mut config: libcec_configuration) -> Result<CecConnection> {
        let conn: libcec_connection_t;
        unsafe {
            conn = libcec_initialise(&mut config);
        }
        if conn as usize == 0 {
            Err(Error::LibInitFailed)
        } else {
            Ok(CecConnection { conn, config })
        }
    }

    pub fn open(&self, port: &CStr, timeout: u32) -> Result<()> {
        {
            let ret: ::std::os::raw::c_int;
            unsafe {
                ret = libcec_open(self.conn, port.as_ptr(), timeout);
            }
            if ret == 0 {
                return Err(Error::AdapterOpenFailed);
            }
        }

        // let mut handle: CallbackHandle;
        {
            let ret: ::std::os::raw::c_int;
            unsafe {
                ret = libcec_enable_callbacks(
                    self.conn,
                    std::ptr::null_mut(),
                    self.config.callbacks,
                );
            }
            if ret == 0 {
                return Err(Error::CallbackRegistrationFailed);
            }
        }
        Ok(())
    }

    pub fn transmit(&self, command: cec_command) -> Result<()> {
        let ret: ::std::os::raw::c_int;
        unsafe { ret = libcec_transmit(self.conn, &command) }
        if ret == 0 {
            return Err(Error::TransmitFailed);
        }
        Ok(())
    }
}

impl Drop for CecConnection {
    fn drop(&mut self) {
        unsafe {
            libcec_close(self.conn);
            libcec_destroy(self.conn);
        }
    }
}

impl libcec_configuration {
    pub fn new(
        activate_source : bool,
        device_types: cec_device_type_list,
        callbacks: &'static mut ICECCallbacks,
    ) -> libcec_configuration {
        let mut cfg: libcec_configuration = Default::default();
        cfg.deviceTypes = device_types;
        cfg.callbacks = callbacks;
        cfg.bActivateSource = if activate_source { 1 } else { 0 };
        cfg
    }
}

impl Default for libcec_configuration {
    fn default() -> Self {
        let mut cfg: libcec_configuration;
        unsafe {
            cfg = mem::zeroed::<libcec_configuration>();
            libcec_clear_configuration(&mut cfg);
        }
        cfg.clientVersion = libcec_version::LIBCEC_VERSION_CURRENT as u32;
        cfg.bActivateSource = 0;
        cfg
    }
}

impl Default for cec_device_type_list {
    fn default() -> Self {
        Self {
            types: [
                cec_device_type::RESERVED,
                cec_device_type::RESERVED,
                cec_device_type::RESERVED,
                cec_device_type::RESERVED,
                cec_device_type::RESERVED,
            ],
        }
    }
}
impl From<Vec<cec_device_type>> for cec_device_type_list {
    fn from(device_types: Vec<cec_device_type>) -> Self {
        let mut devices: cec_device_type_list = Default::default();
        for i in 0..5 {
            device_types.get(i).map(|t| devices.types[i] = *t);
        }
        devices
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        // TODO: libcec_enable_callbacks?
        // connection: libcec_connection_t,
        // cbParam: *mut ::std::os::raw::c_void,
        // callbacks: *mut ICECCallbacks,

        // libcec_start_bootloader? libcec_set_active_source?

        // see https://gitlab.com/teozkr/tv-wol-rs/blob/master/src/cec.rs for advice how to hook into callbacks

        // nice-to-have: impl GIVE_AUDIO_STATUS
        // nice-to-have: report audio status to tv regularly
        // nice to have: regularly get audio status and refresh alsa (libcec_audio_get_status)
    }
}
