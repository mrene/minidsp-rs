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

The main structure of this is in the JSON schema, every object is optional and only the objects that are set will trigger a change on the device. For this reason, an `index` field is present (on inputs, outputs, peqs) to indicate which entry should change - all of these are 0-index so inputs on the 2x4HD are `[0, 1]`, outputs `[0, 1, 2, 3]` and PEQs go from `0` to `9`.

Here is an example to change the current config preset to 0, set the first PEQ on inputs 0 and 1, and bypass the 2nd PEQ on both inputs.

Note that the `master_status` object is applied before any other setting. It's therefore safe to send a current preset change followed by PEQ updates in the same call.

```json
{
  "master_status": {
    "preset": 1
  },
  "inputs": [
    {
      "index": 0,
      "peq": [
        {
          "index": 0,
          "coeff": { "b0": 1, "b1": 0, "b2": 0, "a1": 0, "a2": 0 },
          "bypass": false
        },
        { "index": 1, "bypass": true }
      ]
    },
    {
      "index": 1,
      "peq": [
        {
          "index": 0,
          "coeff": { "b0": 1, "b1": 0, "b2": 0, "a1": 0, "a2": 0 },
          "bypass": false
        },
        { "index": 1, "bypass": true }
      ]
    }
  ]
}
```
