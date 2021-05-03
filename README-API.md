A default installation will serve an HTTP API on localhost. Users can edit /etc/minidsp/config.toml in order to change the server's bind address.

`minidspd` will attach to any compatible devices it can find (WI-DGs should be automatically discovered).

This document is a summary of what the API can do, until there's a proper openapi spec.

## API Overview
#### GET /devices
JSON Schema URL: /devices/get.schema

Gets the list of detected devices.

```json
[
  {
    "url": "usb:0001%3A0005%3A04?vid=2752&pid=0011",
    "version": {
      "hw_id": 10,
      "dsp_version": 100,
      "serial": 965535
    },
    "product_name": "2x4HD"
  }
]
```

#### GET /devices/0
JSON Schema URL: /devices/0/get.schema 

Get details about the first device.
```json
{
  "master": {
    "preset": 0,
    "source": "Toslink",
    "volume": -27.0,
    "mute": false
  },
  "available_sources": [
    "analog",
    "toslink",
    "usb"
  ],
  "input_levels": [
    -28.04515,
    -28.10996
  ],
  "output_levels": [
    -57.139038,
    -60.7054,
    -120.0,
    -120.0
  ]
}
```

#### POST /devices/0
JSON Schema URL: /devices/0/post.schema

Changes master status (current preset, source, master volume/mute)

#### POST /devices/0/config

JSON Schema URL: /devices/0/config/post.schema 

Changes *any* parameters, including master status.
