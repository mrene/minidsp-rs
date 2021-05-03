use serde::{Deserialize, Serialize};
/// Main configuration file
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// HTTP endpoints exposing a JSON API, and WebSocket raw packet transport
    pub http_server: Option<HttpServer>,

    /// TCP servers, used for accepting connection from the official applications.
    /// Because these applications don't have the ability to specify the port number, different
    /// local addresses must be used to represent multiple devices.
    #[serde(rename = "tcp_server")]
    pub tcp_servers: Vec<TcpServer>,

    /// Devices that are always available, independent of the discovery process.
    /// If a remote device is not being discovered, it can be added to this list.
    #[serde(rename = "static_device")]
    pub static_devices: Vec<StaticDevice>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http_server: Some(HttpServer {
                bind_address: Some("0.0.0.0:5380".to_string()),
            }),
            tcp_servers: Vec::new(),
            static_devices: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub struct StaticDevice {
    /// URL to use when connecting to this device. Use `minidsp probe` to generate it.
    pub url: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpServer {
    /// Address used to bind the listening socket accepting HTTP connections
    pub bind_address: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TcpServer {
    /// If set, matches the given device serial number
    pub device_serial: Option<u32>,

    // If set, uses the specified device index when accepting connections
    // If none of `device_serial` or `device_index` are set, the first usb
    // device found will be used.
    pub device_index: Option<usize>,

    /// Bind address for this server, if unset, defaults to 0.0.0.0:5333
    pub bind_address: Option<String>,

    // If set, advertise the given IP address using UDP broadcast frames compatible with the mobile apps
    pub advertise: Option<Advertise>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Advertise {
    /// Avertise the given IP address using UDP broadcast frames compatible with the mobile apps
    pub ip: String,

    /// Defines the name used in the advertisement packets
    pub name: String,
}
