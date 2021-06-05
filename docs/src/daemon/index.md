# Daemon
An optional service is packaged in order to help with applications requiring constant access to devices.

# Architecture
A list of local and remote (WI-DG) devices is maintained. Upon discovering a device, it is probed for its type and model, then made available to applications. 

## Static devices
Remote devices that are advertised are usually discovered, but if running into some issue it's possible to add a static device and make it always available.

`minidsp probe --net` will output the urls for the devices it finds. 

#### Example
Add a WI-DG device while bypassing auto-discovery
```toml
[[static_device]]
url = "tcp://192.168.1.2:5333"
```

## Configuration
The following default configuration file is installed in `/etc/minidsp/config.toml`:
```toml
{{#include ../../config.example.toml}}
```