# Getting Started
minidsp-rs can run on a variety of systems. To get started, the easiest is to download a package for your distribution, or a precompiled binary for your operating system.

## Installation
### From packages
[In the releases section](https://github.com/mrene/minidsp-rs/releases), pre-built packages are available for different platforms.

Debian packages are available for:
- armhf: Tested on raspbian (Raspberry PI, including the rpi0)
- x86_64 Debian / Ubuntu variants

Single binary distribution are also provided for common operating systems:
- Linux: minidsp.x86_64-unknown-linux-gnu.tar.gz
- MacOS: minidsp.x86_64-apple-darwin.tar.gz
- Windows: minidsp.x86_64-pc-windows-msvc.zip

### From source
If you don't have rust setup, the quickest way to get started is with [rustup](https://rustup.rs/). This is preferred over install rust via your distro's package manager because these are often out of date and will have issues compiling recent code.

```bash
cargo build --release --bin minidsp
# The binary will then available as target/release/minidsp

# If you want to build a debian package
cargo install cargo-deb
cargo deb
# Then look under target/debian/
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