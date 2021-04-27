//! Device discovery integrated as the builder pattern

use futures::{StreamExt, TryStreamExt};
use minidsp_protocol::device;
use url2::Url2;

use crate::{
    client::Client,
    device::DeviceKind,
    transport::{hid, Multiplexer, Openable},
    utils::decoder::Decoder,
    MiniDSP, MiniDSPError,
};
use std::{collections::HashMap, net::ToSocketAddrs, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

///
#[derive(Default)]
pub struct Builder {
    log_filename: Option<PathBuf>,
    log_console: Option<u8>,
    kind: Option<DeviceKind>,

    /// The candidate device pool, devices get added when their helper methods
    /// detect matching ones. (url -> openable)
    candidate_devices: HashMap<String, Box<dyn Openable>>,
}

impl Builder {
    pub fn new() -> Self {
        Default::default()
    }

    fn extend_hid_device(&mut self, devices: impl IntoIterator<Item = hid::Device>) {
        let devices = devices.into_iter().map(|dev| {
            (
                Openable::to_string(&dev),
                Box::new(dev) as Box<dyn Openable>,
            )
        });

        self.candidate_devices.extend(devices);
    }

    /// Add all local devices matching known minidsp vendor and product ids
    pub fn with_default_usb(mut self) -> Result<Self, hid::HidError> {
        let api = hid::initialize_api()?;
        self.extend_hid_device(hid::discover(&api)?);
        Ok(self)
    }

    /// Add all local devices matching `vid` and `pid`
    pub fn with_usb_product_id<T: Into<Option<u16>>>(
        mut self,
        vid: u16,
        pid: T,
    ) -> Result<Self, hid::HidError> {
        let api = hid::initialize_api()?;
        let pid = pid.into();
        self.extend_hid_device(hid::discover_with(&api, |dev| {
            vid == dev.vendor_id() && (pid.is_none() || pid == Some(dev.product_id()))
        })?);
        Ok(self)
    }

    /// Adds a single usb device by path
    pub fn with_usb_path(mut self, path: &str) -> Self {
        self.extend_hid_device(Some(hid::Device {
            id: None,
            path: Some(path.into()),
        }));
        self
    }

    /// Adds a remote compat tcp server, or a wi-dg device
    pub fn with_tcp<T: ToSocketAddrs>(mut self, sockaddr: T) -> std::io::Result<Self> {
        self.candidate_devices
            .extend(sockaddr.to_socket_addrs()?.map(|sa| {
                let url = format!("tcp://{}", sa);
                let url2 = Url2::parse(&url);
                (url, Box::new(url2) as Box<dyn Openable>)
            }));
        Ok(self)
    }

    /// Adds a device by url
    pub fn with_url(mut self, s: &str) -> Self {
        let url2 = Url2::parse(s);
        self.candidate_devices
            .insert(s.into(), Box::new(url2) as Box<dyn Openable>);
        self
    }

    /// Activates console logging at the given level, optionally logging all sent and received frames to a file
    pub fn with_logging<T, U>(mut self, level: u8, filename: T) -> Self
    where
        T: Into<Option<U>>,
        U: Into<PathBuf>,
    {
        self.log_console.replace(level);
        self.log_filename = T::into(filename).map(U::into);
        self
    }

    /// Do not probe the device to identify what hardware it is, and use the specified DeviceKind instead.
    pub fn force_device_kind(mut self, kind: DeviceKind) -> Self {
        self.kind.replace(kind);
        self
    }

    /// Probe all candidate devices
    pub async fn probe(self) -> Result<Vec<MiniDSP<'static>>, MiniDSPError> {
        let Self {
            log_filename,
            log_console,
            kind,
            candidate_devices,
        } = self;

        // Attempt to instantiate every candidate device
        futures::stream::iter(candidate_devices)
            .then(|(_, dev)| {
                let log_filename = log_filename.clone();

                async move {
                    let mut transport = dev.open().await?;
                    let mut decoder: Option<Arc<Mutex<Decoder>>> = None;
                    // Apply any logging options
                    if log_console.is_some() || log_filename.is_some() {
                        let wrapped = crate::logging::transport_logging(
                            transport,
                            log_console.unwrap_or_default(),
                            log_filename.clone(),
                        );
                        decoder = wrapped.0;
                        transport = wrapped.1;
                    }

                    // Convert to a service
                    // FIXME: This is convoluted, should be a part of Client
                    let mplex = Multiplexer::from_transport(transport);
                    let svc = Arc::new(Mutex::new(mplex.to_service()));

                    // Probe the device for its hw id and serial
                    let client = Client::new(svc);
                    let device_info = client.get_device_info().await?;
                    let device = match kind {
                        None => device::probe(&device_info),
                        Some(k) => device::by_kind(k),
                    };

                    if let Some(decoder) = decoder {
                        let mut decoder = decoder.lock().await;
                        decoder.set_name_map(device.symbols.iter().copied());
                    }

                    let dsp = MiniDSP::from_client(client, device, device_info);
                    Ok::<_, MiniDSPError>(dsp)
                }
            })
            .try_collect()
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[tokio::test]
    async fn test_builder() {
        // Device selection. Options are *additive*
        let b = Builder::new()
            // Discover any matching usb devices
            .with_default_usb()
            .unwrap()
            // Discover usb devices matching the given vid+pid
            .with_usb_product_id(0x2752, 0x0011)
            .unwrap()
            // Use a single usb device by path
            .with_usb_path("usb:")
            // Connect via tcp (yields one device)
            .with_tcp("127.0.0.1:5333")
            .unwrap()
            // Connect to a specific device by url (hid,tcp,websocket,etc.)
            .with_url("ws://127.0.0.1:5380/devices/0")
            // Console transport logging
            .with_logging(0, &PathBuf::from("file.log"))
            // Probing/device options
            .force_device_kind(DeviceKind::M2x4Hd)
            // Probe all matching devices
            .probe()
            .await;
    }
}
