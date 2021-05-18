# Compat TCP Server
This application provides a tcp server compatible with the official apps by forwarding USB HID frames over a TCP connection. Since they operate at the frame level, they do not require the device type to be supported in order to work with the existing applications.

## Multiple devices
When working with multiple devices, it's necessary to bind to different ip addresses because the official apps do not support specifying a port number in the address field.

Multiple `[[tcp_server]]` blocks can be defined, the `device_serial` option can be used to map each server to a specific device. If you'd rather trust the order at which USB devices are found, then `device_index` can be set to the 0-based index.

## Advertisement
Certain minidsp apps can discover devices on the local network, enabling the advertise option makes these apps find this server instance. Note that to keep OS-specific details to a minimum, the ip address of the server needs to be included in the advertisement config.

```toml
advertise = { ip = "192.168.1.100", name="Living Room Pi" }
```


## Configuration

```toml
[[tcp_server]]
bind_address = "127.0.0.1:5333"
# Replace the previous line by this one in order to 
# use the plugin app from another machine
# bind_address = "0.0.0.0:5333"

# If multiple minidsp devices are used, the following options 
# can be used to expose multiple plugin-compatible 
# servers for each devices.
# device_index = 0 # Use the first device available
# device_serial = 912345 # Use the device with this serial number

# Advertise the following address for the official minidsp mobile app
# advertise = { ip = "192.168.1.100", name="Living Room Pi" }
```