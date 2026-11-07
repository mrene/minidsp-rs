# MiniDSP Controller: web configuration fork

This is a fork of [minidsp-rs](https://github.com/mrene/minidsp-rs). It turns the miniDSP into a
device you configure from a web browser, instead of one that needs a computer running miniDSP
Device Console plugged into it.

> **Current state: macOS, with the miniDSP connected to the Mac over USB.**
> The Mac runs `minidspd`, which talks to the miniDSP over USB and serves the web interface.
> You open the interface in a browser on that Mac, or on a phone or tablet on the same network.
> Raspberry Pi support comes later.
>
> So far everything has been tested against simulated devices on macOS, not yet against a real
> miniDSP (including the Flex HTx). Expect rough edges on real hardware.

## Goals

- **Web-based configuration.** Change every setting from a browser on a phone, tablet or laptop.
  A small always-on host stays connected to the miniDSP and serves the admin interface: a Mac
  today, a Raspberry Pi next. Device Console is no longer needed to make adjustments.
- **Audio over TOSLINK, control over USB.** Audio arrives from your source over TOSLINK. The USB
  port is used only by the host that runs the admin web interface. The input can be locked to
  TOSLINK so it isn't switched by accident.
- **More than 4 presets.** The miniDSP holds 4 presets. This fork keeps an unlimited library of
  presets as files on the host and writes any of them into a device slot when you need it.
  Loading two presets into two slots gives instant A/B switching for listening tests.
- **Renaming inputs and outputs.** The miniDSP Flex HTx labels its channels for home theatre use.
  Every input and output can be given your own name ("Left woofer", "Turntable") to make the
  device easier to set up for other uses, such as active speakers, a studio or a multi-room
  system.

## What works

- Web interface served by `minidspd`: master volume, mute, preset, source and Dirac, live level
  meters, per-channel gain, mute, polarity and delay, and the routing matrix
- Parametric EQ with a frequency-response plot (drag bands on the graph, or import REW filters),
  crossovers, compressor and FIR filters
- Preset library with save, update, load into any slot, export/import and A/B switching. Each
  preset keeps a full, timestamped history with undo and rollback, and deleted presets can be
  restored
- Source and preset locks, enforced by `minidspd` for every client
- Custom names for every input and output, shared by every browser
- Linked channels (e.g. left and right woofer) that share EQ, crossover, gain and more
- Branding: your own name, logo, accent color and a choice of five font pairs
- An [MCP server](mcp/README.md) so an AI assistant such as Claude can make adjustments,
  including loading REW-generated EQ
- A [USB capture tool](devtools/usb-capture/README.md) for recording how Device Console talks to
  the device on macOS

## Set up on a Mac

You need [Rust](https://rustup.rs/) and [Node.js](https://nodejs.org/) (22.18 or later). This
fork has no pre-built packages yet; the upstream packages below don't include its features.

1. Connect the miniDSP to the Mac with a USB cable, and quit Device Console if it's running, so
   the two don't send commands to the device at the same time.
2. From the repository root, build the web interface and start `minidspd`:
   ```sh
   cd gui && npm install && npm run build && cd ..
   cargo run --release --bin minidspd -- --config gui/minidspd-usb.toml
   ```
3. Open http://127.0.0.1:5380.

**The source and the active preset start locked**, so they can't be changed by accident. Untick
**Lock** next to them in the header to change them.

By default only the Mac itself can open the interface. To use it from a phone or another
computer, set `bind_address = "0.0.0.0:5380"` in `gui/minidspd-usb.toml` and open
`http://<your Mac's address>:5380`. The interface has no login, so only do this on a network you
trust.

**No miniDSP at hand?** `gui/dev/minidspd.toml` starts two simulated devices instead:
`cargo run --bin minidspd -- --config gui/dev/minidspd.toml`.

### Where your data is kept

Presets and their history, channel names, links, locks and branding are files in
`~/.local/share/minidsp/` (presets under `presets/`). Back up that folder to keep them. To keep
them elsewhere, set `presets_dir` in the config; the other files go in the folder above it.

## Not done yet

- **Testing on real hardware, including the Flex HTx.**
- **Reading settings back from the device.** The miniDSP protocol as currently understood can't
  report per-channel settings (EQ, gains, crossovers and so on), so the interface shows what it
  last sent and marks the rest as unknown. It also won't show changes made through the MCP server,
  Device Console or another app. The plan is to capture how Device Console reads settings over
  USB on macOS and add that to the library.
- **Raspberry Pi support** for the web interface. `minidspd` itself already builds for the Pi.

---

The rest of this README is the upstream minidsp-rs documentation. It still applies to the
command-line tool and the daemon, but its pre-built packages don't include this fork's features.

## About minidsp-rs
[![GitHub release](https://img.shields.io/github/v/release/mrene/minidsp-rs?include_prereleases)](https://github.com/mrene/minidsp-rs/releases) [![Documentation](https://img.shields.io/badge/docs-online-success)](https://minidsp-rs.pages.dev/) [![Discord](https://img.shields.io/discord/850873168558424095?label=discord&logo=discord)](https://discord.gg/XGHmrcDumf)

minidsp-rs is an alternative control software for certain MiniDSP products. It exposes most (if not all) of the available configuration parameters in a command line package, with an optional HTTP API in order to integrate with custom DIY audio projects. It can run on a variety of systems with a minimal memory footprint.

## Installation
Pre-built packages and binaries are available [in the project's releases section](https://github.com/mrene/minidsp-rs/releases). 

Debian (`.deb`) packages are available for:
- armhf: Tested on raspbian (Raspberry PI, including the rpi0)
- x86_64 Debian / Ubuntu variants

Single binary builds are also provided for common operating systems:
- Linux: minidsp.x86_64-unknown-linux-gnu.tar.gz
- MacOS: minidsp.x86_64-apple-darwin.tar.gz
- Windows: minidsp.x86_64-pc-windows-msvc.zip


### Building from source
This is only required if you want to make changes to minidsp-rs. If you're just trying to control your device, use one of the [pre-built packages](https://github.com/mrene/minidsp-rs/releases)

If you don't have rust setup, the quickest way to get started is with [rustup](https://rustup.rs/). This is preferred over install rust via your distro's package manager because these are often out of date and will have issues compiling recent code.

```bash
cargo build --release --bin minidsp
# The binary will then available as target/release/minidsp

# If you want to build a debian package
cargo install cargo-deb
cargo deb
# Then look under target/debian/
```

## Usage
See the [complete documentation](https://minidsp-rs.pages.dev/) for more examples.

Running the command without any parameters will return a status summary, in this form:

```
$ minidsp 
MasterStatus { preset: 0, source: Toslink, volume: Gain(-8.0), mute: false, dirac: false }
Input levels: -61.6, -57.9
Output levels: -67.9, -71.6, -120.0, -120.0
```

## Useful commands
```
# Set input source to toslink
minidsp source toslink

# Set master volume to -30dB
minidsp gain -- -30

# Activate the 2nd configuration setting (indexing starts at 0)
minidsp config 1
```

## Supported devices
These device support the full feature set. See the [documentation](https://minidsp-rs.pages.dev/devices) for a more complete list.

- miniDSP 2x4HD
- miniDSP Flex
- DDRC-24
- DDRC-88A/D
- miniSHARC series
- miniDSP 2x8/8x8/4x10/10x10
- nanoDIGI 2x8
- SHD series
- C-DSP 8x12 v2
