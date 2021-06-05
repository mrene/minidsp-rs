# Getting Started

[![GitHub release](https://img.shields.io/github/v/release/mrene/minidsp-rs?include_prereleases)](https://github.com/mrene/minidsp-rs/releases) [![Discord](https://img.shields.io/discord/850873168558424095?label=discord&logo=discord)](https://discord.gg/XGHmrcDumf)

minidsp-rs is an alternative control software for certain MiniDSP products. It exposes most/all of the available configuration parameters in a command line package, with an optional HTTP API in order to integrate with custom DIY audio projects. It can run on a variety of systems with a minimal memory footprint.

## Installation
Pre-built packages and binaries are available [in the project's releases section](https://github.com/mrene/minidsp-rs/releases). 

Debian (`.deb`) packages are available for:
- armhf: Tested on raspbian (Raspberry PI, including the rpi0)
- x86_64 Debian / Ubuntu variants

Single binary builds are also provided for common operating systems:
- Linux: minidsp.x86_64-unknown-linux-gnu.tar.gz
- MacOS: minidsp.x86_64-apple-darwin.tar.gz
- Windows: minidsp.x86_64-pc-windows-msvc.zip


## Useful commands
```
# Set input source to toslink
minidsp source toslink

# Set master volume to -30dB
minidsp gain -- -30

# Activate the 2nd configuration setting (indexing starts at 0)
minidsp config 1
```

## Building from source
If you don't have rust setup, the quickest way to get started is with [rustup](https://rustup.rs/). This is preferred over install rust via your distro's package manager because these are often out of date and will have issues compiling recent code.

```bash
cargo build --release --bin minidsp
# The binary will then available as target/release/minidsp

# If you want to build a debian package
cargo install cargo-deb
cargo deb
# Then look under target/debian/
```
