//! Device discovery integrated as the builder pattern

use std::{collections::HashMap, net::ToSocketAddrs, path::PathBuf, pin::Pin, sync::Arc};

use futures::{Stream, StreamExt};
use minidsp_protocol::{
    device::{self, Device},
    DeviceInfo,
};
use tokio::sync::Mutex;
use url2::Url2;

#[cfg(feature = "hid")]
use crate::transport::hid;
use crate::{
    client::Client,
    device::DeviceKind,
    transport::{self, Hub, Multiplexer, Openable},
    utils::decoder::Decoder,
    MiniDSP, MiniDSPError,
};

/// Discovers, probes and instantiate device instances.
///
/// - Configure what devices to probe using the `with_` methods
/// - Consume builder into a vec of transports using [`probe`]
/// - Get an instance of [`MiniDSP`] using [`to_minidsp`]
#[derive(Default)]
pub struct Builder {
    /// The candidate device pool, devices get added when their helper methods
    /// detect matching ones. (url -> openable)
    candidate_devices: HashMap<String, Box<dyn Openable>>,
    options: DeviceOptions,
}

#[derive(Default, Clone)]
pub struct DeviceOptions {
    log_filename: Option<PathBuf>,
    log_console: Option<u8>,
    kind: Option<DeviceKind>,
}

/// TODO: Copied from minidspd

pub struct DeviceHandle {
    pub url: String,

    // Frame-level multiplexer
    pub transport: Hub,

    // Probed hardware id and dsp version
    pub device_info: DeviceInfo,

    // Device spec structure indicating the address of every component
    pub device_spec: &'static Device,
}

impl DeviceHandle {
    pub fn to_minidsp(&self) -> Option<MiniDSP<'static>> {
        let transport = self.transport.try_clone()?;
        let multiplexer = Multiplexer::from_transport(transport);
        let client = Client::new(Arc::new(Mutex::new(multiplexer.to_service())));
        let dsp = MiniDSP::from_client(client, self.device_spec, self.device_info);
        Some(dsp)
    }

    pub fn to_hub(&self) -> Option<Hub> {
        self.transport.try_clone()
    }
}

impl Builder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Uses devices managed by a remote instance of minidspd
    pub async fn with_http(&mut self, s: &str) -> Result<&mut Self, transport::ws::Error> {
        let url = Url2::try_parse(s)?;
        self.candidate_devices.extend(
            transport::ws::discover(&url)
                .await?
                .into_iter()
                .map(|url| (url.to_url(), Box::new(url) as Box<dyn Openable>)),
        );
        Ok(self)
    }

    /// Uses devices managed by a local instance of minidspd
    #[cfg(target_family = "unix")]
    pub async fn with_unix_socket(
        &mut self,
        socket_path: &str,
    ) -> Result<&mut Self, transport::ws::Error> {
        self.candidate_devices.extend(
            transport::ws::discover_unix(socket_path)
                .await?
                .into_iter()
                .map(|device| (device.to_url(), Box::new(device) as Box<dyn Openable>)),
        );
        Ok(self)
    }

    #[cfg(feature = "hid")]
    fn extend_hid_device(&mut self, devices: impl IntoIterator<Item = hid::Device>) {
        let devices = devices
            .into_iter()
            .map(|dev| (Openable::to_url(&dev), Box::new(dev) as Box<dyn Openable>));

        self.candidate_devices.extend(devices);
    }

    /// Add all local devices matching known minidsp vendor and product ids
    #[cfg(feature = "hid")]
    pub fn with_default_usb(&mut self) -> Result<&mut Self, hid::HidError> {
        let api = hid::initialize_api()?;
        let mut api = api.lock().unwrap();
        self.extend_hid_device(hid::discover(&mut api)?);
        Ok(self)
    }

    /// Add all local devices matching `vid` and `pid`
    #[cfg(feature = "hid")]
    pub fn with_usb_product_id<T: Into<Option<u16>>>(
        &mut self,
        vid: u16,
        pid: T,
    ) -> Result<&mut Self, hid::HidError> {
        let api = hid::initialize_api()?;
        let mut api = api.lock().unwrap();
        let pid = pid.into();
        self.extend_hid_device(hid::discover_with(&mut api, |dev| {
            vid == dev.vendor_id() && (pid.is_none() || pid == Some(dev.product_id()))
        })?);
        Ok(self)
    }

    /// Adds a single usb device by path
    #[cfg(feature = "hid")]
    pub fn with_usb_path(&mut self, path: &str) -> &mut Self {
        self.extend_hid_device(Some(hid::Device {
            id: None,
            path: Some(path.into()),
        }));
        self
    }

    /// Adds a remote compat tcp server, or a wi-dg device
    pub fn with_tcp<T: ToSocketAddrs>(&mut self, sockaddr: T) -> std::io::Result<&mut Self> {
        self.candidate_devices
            .extend(sockaddr.to_socket_addrs()?.map(|sa| {
                let url = format!("tcp://{}", sa);
                let url2 = Url2::parse(&url);
                (url, Box::new(url2) as Box<dyn Openable>)
            }));
        Ok(self)
    }

    /// Adds a device by url
    pub fn with_url(&mut self, s: &str) -> Result<&mut Self, url2::Url2Error> {
        let url2 = Url2::try_parse(s)?;
        self.candidate_devices
            .insert(s.into(), Box::new(url2) as Box<dyn Openable>);
        Ok(self)
    }

    /// Activates console logging at the given level, optionally logging all sent and received frames to a file
    pub fn with_logging<T>(&mut self, level: u8, filename: T) -> &mut Self
    where
        T: Into<Option<PathBuf>>,
    {
        self.options.log_console.replace(level);
        self.options.log_filename = T::into(filename);
        self
    }

    /// Do not probe the device to identify what hardware it is, and use the specified DeviceKind instead.
    pub fn force_device_kind(&mut self, kind: DeviceKind) -> &mut Self {
        self.options.kind.replace(kind);
        self
    }
    /// Probe all candidate devices
    pub fn probe(self) -> Pin<Box<impl Stream<Item = Result<DeviceHandle, MiniDSPError>>>> {
        let Self {
            options,
            candidate_devices,
        } = self;

        let options = Arc::new(options);

        // Attempt to instantiate every candidate device
        Box::pin(
            futures::stream::iter(candidate_devices).then(move |(key, dev)| {
                let options = options.clone();

                async move {
                    let mut transport = dev.open().await?;
                    let mut decoder: Option<Arc<Mutex<Decoder>>> = None;
                    // Apply any logging options
                    if options.log_console.is_some() || options.log_filename.is_some() {
                        let wrapped = crate::logging::transport_logging(
                            transport,
                            options.log_console.unwrap_or_default(),
                            options.log_filename.clone(),
                        );
                        decoder = wrapped.0;
                        transport = wrapped.1;
                    }

                    let hub = Hub::new(transport);

                    // Convert to a service
                    // FIXME: This is convoluted, should be a part of Client
                    // Probe the device for its hw id and serial
                    let mplex = Multiplexer::from_transport(
                        hub.try_clone().ok_or(MiniDSPError::TransportClosed)?,
                    );
                    let svc = Arc::new(Mutex::new(mplex.to_service()));
                    let client = Client::new(svc);

                    let device_info = client.get_device_info().await?;
                    let device_spec = match options.kind {
                        None => device::probe(&device_info),
                        Some(k) => device::by_kind(k),
                    };

                    #[cfg(feature = "devices")]
                    if let Some(decoder) = decoder {
                        let mut decoder = decoder.lock().await;
                        decoder.set_name_map(device_spec.symbols.iter().copied());
                    }
                    Ok::<_, MiniDSPError>(DeviceHandle {
                        url: key,
                        transport: hub,
                        device_info,
                        device_spec,
                    })
                }
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[tokio::test]
    async fn test_builder() {
        // Device selection. Options are *additive*
        let mut b = Builder::new();
        b
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
            .with_url("ws://127.0.0.1:5380/devices/0/ws")
            .unwrap()
            // Console transport logging
            .with_logging(0, PathBuf::from("file.log"))
            // Probing/device options
            .force_device_kind(DeviceKind::M2x4Hd);
        // Probe all matching devices
        let _ = b.probe().next().await.unwrap();
    }
}
