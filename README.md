# MiniDSP Controller
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
