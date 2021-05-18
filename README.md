# Open Source MiniDSP Controller
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
- miniSHARC series
