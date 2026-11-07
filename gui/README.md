# minidsp web interface

A browser interface for `minidspd`, styled like measurement software.

**Current state: macOS, with the miniDSP connected to the Mac over USB.** The Mac runs
`minidspd`, which talks to the miniDSP and serves this interface:

```
[browser on the Mac, phone or tablet] --HTTP / websocket--> [minidspd on the Mac] --USB--> [miniDSP]
```

## Features

- **Master:** volume, mute, preset (1–4), source and Dirac, with live level meters
- **Channels:** gain, mute, polarity and delay, plus the routing matrix
- **Parametric EQ** with a frequency-response plot: drag bands on the graph, scroll to change Q,
  or import REW's miniDSP biquad export
- **Crossovers** (Butterworth 6–48 dB/oct, Linkwitz-Riley 12–48 dB/oct), **compressor**, and
  **FIR filters** loaded from WAV or text files
- **Overview** plot of every output's combined filter response
- **Presets:** an unlimited library on the Mac, loaded into any of the device's 4 slots, with A/B
  switching, rename, notes, export/import, a timestamped version history with undo and rollback,
  and restore of deleted presets
- **Locks** on the source and the active preset (on by default), enforced by `minidspd`
- **Custom names** for every input and output
- **Linked channels**, e.g. left and right woofer sharing EQ, crossover and gain
- **Settings → Branding:** your own name, logo and accent color

Names, links, locks, branding and presets are stored by `minidspd`, so every browser sees the same.

## Run it on a Mac with a miniDSP over USB

Requires Rust and Node.js 22.18 or later. Quit Device Console first. From the repository root:

```sh
# 1. Build the interface (output goes to gui/dist)
cd gui && npm install && npm run build && cd ..

# 2. Start minidspd for the USB-connected miniDSP; it also serves gui/dist
cargo run --release --bin minidspd -- --config gui/minidspd-usb.toml

# 3. Open http://127.0.0.1:5380
```

The source and preset start locked; untick **Lock** next to them in the header to change them.

To open the interface from a phone or another computer, set `bind_address = "0.0.0.0:5380"` in
`gui/minidspd-usb.toml`. There is no login, so only do this on a network you trust.

### Without a miniDSP

`gui/dev/minidspd.toml` starts two simulated devices, and keeps its data in `gui/dev/`:

```sh
cargo run --bin minidspd -- --config gui/dev/minidspd.toml
```

## Configuration

The daemon's config options used by the interface:

```toml
[http_server]
bind_address = "127.0.0.1:5380"   # "0.0.0.0:5380" to reach it from other devices
static_dir = "gui/dist"           # the built interface

# Where presets are kept; channel names, links, locks and branding go in the folder above.
# Default: ~/.local/share/minidsp/presets
# presets_dir = "/Users/you/minidsp/presets"
```

Back up `~/.local/share/minidsp/` (or your `presets_dir` and the folder above it) to keep your
presets, their history and your settings.

## Working on the interface

Keep the daemon running, then in another terminal:

```sh
cd gui && npm run dev
```

Open the URL Vite prints (http://localhost:5173). Edits reload instantly, and API and websocket
calls are proxied to the daemon on port 5380 (override with `MINIDSPD_URL`).

`npm run check` type-checks the project and `npm test` runs the filter-math unit tests.

## Limitations

- miniDSP devices can't report per-channel settings (gain, mute, delay, polarity, routing, EQ,
  crossover, compressor, FIR). The interface remembers what it last sent, separately for each of
  the device's preset slots, in the browser. A dash or "?" means unknown. Changes made from
  another browser, Device Console or the MCP server won't show up. Plots treat unknown filters as
  flat.
- A saved preset only contains settings the interface knows; loading it leaves the unknown ones
  as they are in the target slot. The Presets page says how many are unknown.
- Master volume, mute, source, preset, Dirac and levels are read from the device and update live.
- "Reset to flat" on the FIR loads a single unit impulse. Clearing the filter the library's way
  would load all-zero taps, which silences the output.
- Compressor value ranges in the interface are estimates; check them against Device Console.
- Only tested against simulated devices so far.
