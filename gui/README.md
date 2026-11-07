# minidsp web interface

A browser interface for `minidspd`, styled like measurement software:

- Master volume, mute, preset, source and Dirac, with live level meters
- Per channel: gain, mute, polarity and delay, plus the routing matrix
- Parametric EQ with a frequency-response plot: drag bands on the graph, scroll to change Q, or
  import REW's "miniDSP biquad" export
- Crossovers (Butterworth 6–48 dB/oct, Linkwitz-Riley 12–48 dB/oct), compressor, and FIR filters
  loaded from WAV or text files
- An overview plot of every output's combined filter response

The page talks to the daemon's HTTP API and websocket; the daemon talks to the device.

```
[browser] --HTTP / websocket--> [minidspd] --USB / TCP--> [miniDSP]
```

## Try it on macOS with mock devices

Requires Rust and Node.js. From the repository root:

```sh
# 1. Build the interface (output goes to gui/dist)
cd gui && npm install && npm run build && cd ..

# 2. Start the daemon with two mock devices; it also serves gui/dist
cargo run --bin minidspd -- --config gui/dev/minidspd.toml

# 3. Open http://127.0.0.1:5380
```

To use a real device instead, plug it in and remove the `[[static_device]]` sections from
`gui/dev/minidspd.toml`; USB devices are discovered automatically.

## Working on the interface

Keep the daemon running (step 2), then in another terminal:

```sh
cd gui && npm run dev
```

Open the URL Vite prints (http://localhost:5173). Edits reload instantly, and API and websocket
calls are proxied to the daemon on port 5380 (override with `MINIDSPD_URL`).

`npm run check` type-checks the project and `npm test` runs the filter-math unit tests.

## Serving it from minidspd

Point `static_dir` at the build output in the daemon's config:

```toml
[http_server]
bind_address = "127.0.0.1:5380"   # use 0.0.0.0:5380 to reach it from other devices
static_dir = "/path/to/gui/dist"
```

The daemon has no authentication. Only expose it on a network you trust.

## Limitations

- miniDSP devices can't report per-channel settings (gain, mute, delay, polarity, routing, EQ,
  crossover, compressor, FIR). The interface remembers what it last sent, per device, in the
  browser. A dash or "?" means unknown, and changes made from another app or browser won't show
  up. Plots treat unknown filters as flat.
- Master volume, mute, source, preset, Dirac and levels are read from the device and update live.
- "Reset to flat" on the FIR loads a single unit impulse. Clearing the filter the library's way
  would load all-zero taps, which silences the output.
- Compressor value ranges in the interface are estimates; check them against Device Console.
