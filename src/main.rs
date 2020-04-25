extern crate hidapi;

use hidapi::{HidApi, HidDevice};
use std::error::Error;
use std::iter;

extern crate hap;

use hap::characteristic::brightness;
use hap::service::HapService;
use hap::{
    accessory::{television, Category, Information},
    characteristic::{volume, Characteristic, Readable, Updatable},
    transport::{IpTransport, Transport},
    Config, HapType,
};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex};
use minidsp::MiniDSP;
use minidsp::transport;

#[derive(Clone)]get_master_status
struct Mute {
    value: Arc<Mutex<bool>>,
}

impl Mute {
    pub fn new() -> Self {
        Mute {
            value: Arc::new(Mutex::new(false)),
        }
    }
}

impl Readable<bool> for Mute {
    fn on_read(&mut self, hap_type: HapType) -> Option<bool> {
        let val = Some(*self.value.lock().unwrap());
        println!("Read: {:?}", val);
        val
    }
}

impl Updatable<bool> for Mute {
    fn on_update(&mut self, old_val: &bool, new_val: &bool, hap_type: HapType) {
        let mut value = self.value.lock().unwrap();
        *value = *new_val;
        println!("Val: {:?} {:?} {:?}", *old_val, *new_val, hap_type);
    }
}

#[derive(Clone)]
struct Volume {
    value: Arc<Mutex<u8>>,
}

impl Volume {
    pub fn new() -> Self {
        Volume {
            value: Arc::new(Mutex::new(0)),
        }
    }
}

impl Readable<u8> for Volume {
    fn on_read(&mut self, hap_type: HapType) -> Option<u8> {
        let val = Some(*self.value.lock().unwrap());
        println!("read vol: {:?}", val);
        val
    }
}

impl Updatable<u8> for Volume {
    fn on_update(&mut self, old_val: &u8, new_val: &u8, hap_type: HapType) {
        let mut value = self.value.lock().unwrap();
        *value = *new_val;
        println!("update vol: {:?}", *new_val);
    }
}


fn main() {
    // let device = get_minidsp();
    // let x = device.get_master_status().unwrap();

    let mut television = television::new(Information {
        name: "MiniDSP".into(),
        ..Default::default()
    })
    .unwrap();

    television.inner.television.inner.set_hidden(false);

    television.inner.television.inner.brightness = Some(brightness::new());

    let mute = Mute::new();
    let vol = Volume::new();

    television
        .inner
        .speaker
        .inner
        .mute
        .set_readable(mute.clone())
        .unwrap();

    television
        .inner
        .speaker
        .inner
        .mute
        .set_updatable(mute.clone())
        .unwrap();

    let mut volume = volume::new();
    volume.set_readable(vol.clone()).unwrap();
    volume.set_updatable(vol.clone()).unwrap();

    television.inner.speaker.inner.volume = Some(volume);

    television.inner.speaker.set_hidden(false);
    television.inner.speaker.set_primary(true);
    television.inner.television.set_hidden(false);

    let mut ip_transport = IpTransport::new(Config {
        name: "MiniDSP".into(),
        category: Category::Television,
        ..Default::default()
    })
    .unwrap();

    ip_transport.add_accessory(television).unwrap();

    ip_transport.start().unwrap();
}

fn get_minidsp() -> MiniDSP<transport::HID> {
    let hid = HidApi::new().unwrap();
    // for device in hid.device_list() {
    //     println!("{:?} {:?} {:?} {:?}", device.vendor_id(), device.product_id(), device.manufacturer_string(), device.product_string())
    // }
    let (vid, pid) = (0x2752, 0x0011);
    let hid_device = hid.open(vid, pid).unwrap();
    let device = transport::HID::new(hid_device);
    MiniDSP::new(device)
}
