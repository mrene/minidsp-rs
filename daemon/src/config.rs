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

    /// Set to ignore network devices that broadcast advertisement packets (such as the WI-DG).
    /// It's still possible to add them using `[[static_device]]`
    pub ignore_advertisements: bool,

    /// ALSA mixer integration settings (Linux only)
    #[cfg(target_os = "linux")]
    pub alsa_mixer: Option<AlsaMixer>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http_server: Some(HttpServer {
                bind_address: Some("0.0.0.0:5380".to_string()),
                ..Default::default()
            }),
            tcp_servers: Vec::new(),
            static_devices: Vec::new(),
            ignore_advertisements: false,
            #[cfg(target_os = "linux")]
            alsa_mixer: Some(AlsaMixer {
                enabled: true,
                card_name: None,
                control_name: Some("Digital".to_string()),
                sync_interval_ms: None,
                use_virtual_control: true,
                output_device: Some("hw:0".to_string()),
            }),
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

    /// If set, CORS headers will be set to allow the specified origins
    pub allowed_origins: Option<String>,
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

    /// Bind address to use when sending broadcast packets
    pub bind_address: Option<String>,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AlsaMixer {
    /// Enable ALSA mixer integration (Linux only)
    pub enabled: bool,

    /// ALSA card name (e.g., "default", "hw:0")
    /// Defaults to "default" if not specified
    pub card_name: Option<String>,

    /// Control name for the virtual/mapped control
    /// Defaults to "MiniDSP" if not specified
    pub control_name: Option<String>,

    /// Sync interval in milliseconds
    /// Defaults to 100ms if not specified
    pub sync_interval_ms: Option<u64>,

    /// Use virtual control creation (default: true, falls back to mapping if fails)
    /// If false, always maps to existing controls like "Master" or "PCM"
    pub use_virtual_control: bool,

    /// ALSA device to route audio output to (e.g., "hw:0", "default")
    /// This is where audio physically plays from
    /// Defaults to "hw:0" if not specified
    pub output_device: Option<String>,
}
