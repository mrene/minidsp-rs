# HTTP API
On unix platform, the service exposes this though `/tmp/minidsp.sock` automatically. Accessing through the network must be enabled through the configuration file.

The [full OpenAPI specification](/api.html) is available online, and from the service's `/api` endpoint, loading the spec from `/openapi.json`.

## Configuration
A single bind address can be set, note that there is currently no authentication mechanisms in place.

```toml
[http_server]
bind_address = "127.0.0.1:5380"
```


## Quick start
There are a few useful endpoints to get started, the first one in `/devices` and returns a list of available devices, along with their product type and the URL to which the transport is connected (via usb,tcp,etc.)

#### 1. List available devices
```
$ curl http://localhost:5380/devices
```

```json
[
  {
    "url": "tcp://192.168.1.242:5333?name=Living%20Room%20Pi",
    "version": {
      "hw_id": 10,
      "dsp_version": 100,
      "serial": 965535
    },
    "product_name": "2x4HD"
  }
]
```

#### 2. Retrieve master status
```
$ curl http://localhost:5380/devices/0 -d "{ "master_status": { preset: 0 } }"
```

```json
{
  "master": {
    "preset": 1,
    "source": "Toslink",
    "volume": -8.0,
    "mute": false
  },
  "input_levels": [
    -131.38994,
    -131.38994
  ],
  "output_levels": [
    -105.435165,
    -139.5934,
    -105.435165,
    -139.59183
  ]
}
```

#### 3. Set active preset to 0
```bash
curl http://localhost:5380/devices/0/config \
    -H 'Content-Type: application/json' \
    -d '{ "master_status": { "preset": 0 } }'
```


## WebSocket Streaming
Asynchronous updates to the master status are provided through websocket. Upon upgrading, the full current status is provided, followed by updates to properties that have changes. Note that this currently only returns changes done through the IR remote, in order to get more regular updates it's possible to poll the device for changes by passing `poll=true` in the query string. This requests updates every 2 seconds and sends a message if the status summary has changed. 

Here is an example of the master volume being changed via the IR remote:

```
$ websocat ws://127.0.0.1:5380/devices/0
```
```json
{
    "master":{"preset":0,"source":"Toslink","volume":-19.5,"mute":false},
    "input_levels":[-44.400307,-42.08899],
    "output_levels":[-54.93488,-56.41222,-120.0,-120.0]
}
{"master":{"volume":-19.0,"mute":false}}
{"master":{"volume":-18.5,"mute":false}}
{"master":{"volume":-18.0,"mute":false}}
```

Here is example which polls the device for changes, in order to reflect changes done through the plugin, or other applications.
```
$ websocat 'ws://127.0.0.1:5380/devices/0?poll=true'
```
```json
{
    "master":{"preset":0,"source":"Toslink","volume":-19.5,"mute":false},
    "input_levels":[-44.400307,-42.08899],
    "output_levels":[-54.93488,-56.41222,-120.0,-120.0]
}
{"master":{"preset":0,"source":"Toslink","volume":-19.5,"mute":true}}
```

#### Streaming input and output levels
If the `levels` query string param is set, the current input and output levels will be polled from the device at most every 250ms. 
```bash
$ websocat 'ws://127.0.0.1:5380/devices/0?levels=true' | jq
```

```json
{
  "master": {
    "preset": 0,
    "source": "Toslink",
    "volume": -8,
    "mute": false
  },
  "input_levels": [
    -131.50832,
    -131.50832
  ],
  "output_levels": [
    -140.54817,
    -140.1521,
    -120,
    -120
  ]
}
{
  "master": {},
  "input_levels": [
    -131.44533,
    -131.44533
  ],
  "output_levels": [
    -140.41307,
    -140.02238,
    -120,
    -120
  ]
}
{
  "master": {},
  "input_levels": [
    -131.3853,
    -131.3853
  ],
  "output_levels": [
    -140.26598,
    -139.90532,
    -120,
    -120
  ]
}
```


## Updating settings
The main structure of this is in the OpenAPI specifications, every object is optional and only the objects that are set will trigger a change on the device. For this reason, an `index` field is present (on inputs, outputs, peqs) to indicate which entry should change - all of these are 0-index so inputs on the 2x4HD are `[0, 1]`, outputs `[0, 1, 2, 3]` and PEQs go from `0` to `9`.

Note that the `master_status` object is applied before any other setting. It's therefore safe to send a current preset change followed by PEQ updates in the same call.

### Example
Here is an example to change the current config preset to 0, set the first PEQ on inputs 0 and 1, and bypass the 2nd PEQ on both inputs.


```json
POST /devices/0/config

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
