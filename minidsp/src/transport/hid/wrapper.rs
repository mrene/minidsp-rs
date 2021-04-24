use std::ops::Deref;

use hidapi::HidDevice;

/// Wraps an underlying HidDevice, adding Sync+Send
pub struct HidDeviceWrapper {
    pub inner: HidDevice,
}

impl HidDeviceWrapper {
    pub fn new(inner: HidDevice) -> Self {
        HidDeviceWrapper { inner }
    }
}

impl Deref for HidDeviceWrapper {
    type Target = HidDevice;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// hidapi's libusb backend is thread-safe, different threads can read + send and it does its own locking
unsafe impl Sync for HidDeviceWrapper {}
unsafe impl Send for HidDeviceWrapper {}
