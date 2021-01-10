# MiniDSP Command-line interface

This provides a command line interface to control MiniDSP devices. 
It's a complete rewrite from `node-minidsp` and aims to support multiple devices. Feel free to open an issue if you have access to other hardware!


## Installation
### From packages
[In the releases section](https://github.com/mrene/minidsp-rs/releases), there are pre-built packages available for different platforms.

Debian packages are available for:
- armhf: Tested on raspbian (Raspberry PI, including the rpi0)
- x86_64 Debian / Ubuntu variants

Single binary distribution are also provided for common operating systems:
- Linux: minidsp.x86_64-unknown-linux-gnu.tar.gz
- MacOS: minidsp.x86_64-apple-darwin.tar.gz
- Windows: minidsp.x86_64-pc-windows-msvc.tar.gz

### From source
If you don't have rust setup, the quickest way to get started is with [rustup](https://rustup.rs/)


```shell
cargo build --release --bin minidsp
# The binary will then available as target/release/minidsp

# If you want to build a debian package
cargo install cargo-deb
cargo deb
# Then look under target/debian/
```

### From cargo
There is a published crate which is kept in sync with releases, you can install with:
```shell
cargo install minidsp
```

## Usage
```shell
minidsp 0.0.2-dev
Mathieu Rene <mathieu.rene@gmail.com>

USAGE:
    minidsp [OPTIONS] [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -f <file>          Read commands to run from the given filename
        --tcp <tcp>    The target address of the server component [env: MINIDSP_TCP=]
        --usb <usb>    The USB vendor and product id (2752:0011 for the 2x4HD) [env: MINIDSP_USB=]

SUBCOMMANDS:
    config    Set the current active configuration,
    debug     Low-level debug utilities
    gain      Set the master output gain [-127, 0]
    help      Prints this message or the help of the given subcommand(s)
    input     Control settings regarding input channels
    mute      Set the master mute status
    output    Control settings regarding output channels
    probe     Try to find reachable devices
    server    Launch a server usable with `--tcp`, the mobile application, and the official
              client
    source    Set the active input source
```


## Getting started
Running without arguments will print information about the current state:

```shell
$ minidsp 
MasterStatus { preset: 0, source: Toslink, volume: Gain(-36.5), mute: false }
Input levels: -131.4, -131.4
Output levels: -168.0, -168.0, -120.0, -120.0
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

## Input channel configuration
This is where you'd configure routing, gain settings and PEQ for each input

<details>
  <summary>(Click to expand) minidsp input [input-index] [SUBCOMMAND]</summary>
```shell
$ minidsp input --help
minidsp-input
Control settings regarding input channels

USAGE:
    minidsp input <input-index> <SUBCOMMAND>

ARGS:
    <input-index>    Index of the input channel, starting at 0

SUBCOMMANDS:
    gain       Set the input gain for this channel
    help       Prints this message or the help of the given subcommand(s)
    mute       Set the master mute status
    peq        Control the parametric equalizer
    routing    Controls signal routing from this input
```

#### gain / mute
```shell
# Sets input channel 0's gain to -10dB
minidsp input 0 gain -- -10

# Mute input channel 0
minidsp input 0 mute on
```
#### routing
Each output matrix entry has to be enabled in order for audio to be routed. The gain can then be set (in dB) for each entry.

```shell
# Route input channel 0 to output channel 0, boost gain by 6dB
minidsp input 0 routing 0 enable on
minidsp input 0 routing 0 gain 6
```

#### peq
```
$ minidsp input 0 peq --help
minidsp-input-peq
Control the parametric equalizer

USAGE:
    minidsp input <input-index> peq <index> <SUBCOMMAND>

ARGS:
    <index>    Parametric EQ index (all | <id>) (0 to 9 inclusively)

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    bypass    Sets the bypass toggle
    clear     Sets all coefficients back to their default values and un-bypass them
    help      Prints this message or the help of the given subcommand(s)
    import    Imports the coefficients from the given file
    set       Set coefficients
```

The `peq` commands supports broadcasting an operation on multiple peqs. If specifying
an index, the command will only affect a single filter.

Bypass the first peq:
`minidsp output 0 peq 0 bypass on` 

Bypass all peqs:
`minidsp output 0 peq all bypass on`

Importing filters should use the `all` target if the ununsed filter should also be cleared.
`minidsp output 0 preq all import ./file.txt`

</details>

## Output channel configuration
This is where you'd configure the output gain settings, crossovers, PEQs, FIR filters, compressors, phase inversion and delay for each output channel.

<details>
  <summary>(Click to expand) minidsp output [output-index] [SUBCOMMAND]</summary>
The outputs are referenced by index, starting at 0 for the first output.

```shell
$ minidsp output --help

Control settings regarding output channels

USAGE:
    minidsp output <output-index> <SUBCOMMAND>

ARGS:
    <output-index>    Index of the output channel, starting at 0

SUBCOMMANDS:
    compressor    Controls crossovers (2x 4 biquads)
    crossover     Controls crossovers (2x 4 biquads)
    delay         Set the delay associated to this channel
    fir           Controls the FIR filter
    gain          Set the input gain for this channel
    help          Prints this message or the help of the given subcommand(s)
    invert        Set phase inversion on this channel
    mute          Set the master mute status
    peq           Control the parametric equalizer
```

#### Gain
```shell
$ minidsp output 0 gain --help
USAGE:
    minidsp output <output-index> gain <value>

ARGS:
    <value>    Output gain in dB
```

Example usage: `minidsp output 0 gain -- -20`

`--` is used to distinguish negative values from another option

#### PEQ

```
$ minidsp output 0 peq --help
Control the parametric equalizer

USAGE:
    minidsp output <output-index> peq <index> <SUBCOMMAND>

ARGS:
    <index>    Parametric EQ index (all | <id>) (0 to 9 inclusively)

SUBCOMMANDS:
    bypass    Sets the bypass toggle
    clear     Sets all coefficients back to their default values and un-bypass them
    help      Prints this message or the help of the given subcommand(s)
    import    Imports the coefficients from the given file
    set       Set coefficients
```

The `peq` commands supports broadcasting an operation on multiple peqs. If specifying
an index, the command will only affect a single filter.

Bypass the first peq:
`minidsp output 0 peq 0 bypass on` 

Bypass all peqs:
`minidsp output 0 peq all bypass on`

Importing filters should use the `all` target if the ununsed filter should also be cleared.
`minidsp output 0 preq all import ./file.txt`

#### Crossover
```
$ minidsp output 0 crossover --help
Controls crossovers (2x 4 biquads)

USAGE:
    minidsp output <output-index> crossover <group> <index> <SUBCOMMAND>

ARGS:
    <group>    Group index (0 or 1)
    <index>    Filter index (all | 0 | 1 | 3)

SUBCOMMANDS:
    bypass    Sets the bypass toggle
    clear     Sets all coefficients back to their default values and un-bypass them
    help      Prints this message or the help of the given subcommand(s)
    import    Imports the coefficients from the given file
    set       Set coefficients
```

Crossovers are implemented as series biquad filters. There are two groups of 4 biquads per channel. Each group can be bypassed individually.

The command follows the same syntax as the `peq` command, for the exception that you have to specify the group index.

They can be imported in REW's format:
`minidsp output 0 crossover 0 all import ./file.txt`
`minidsp output 0 crossover 1 all import ./file2.txt`

#### FIR
```shell
$ minidsp output 0 fir --help
minidsp-output-fir
Controls the FIR filter

USAGE:
    minidsp output <output-index> fir <SUBCOMMAND>

SUBCOMMANDS:
    bypass    Sets the bypass toggle
    clear     Sets all coefficients back to their default values and un-bypass them
    help      Prints this message or the help of the given subcommand(s)
    import    Imports the coefficients from the given file
    set       Set coefficients
```

Importing FIR filters can be done using a wav file. The file's sampling rate MUST match the device's internal rate. 

`minidsp output 0 fir import ./impulse.wav`
`minidsp output 0 fir bypass off`

#### Delay
```shell
$ minidsp output 0 delay --help
minidsp-output-delay
Set the delay associated to this channel

USAGE:
    minidsp output <output-index> delay <delay>

ARGS:
    <delay>    Delay in milliseconds
```

#### Invert
```
USAGE:
    minidsp output <output-index> invert <value>
```

Example: `minidsp output 0 invert on`

</details>

### Importing filters from Room Eq Wizard (REW)
The `minidsp output n peq` and `minidsp input n peq` commands both support importing from a REW-formatted file. If there are less
filters on the device, the remaining PEQs will be cleared.

```shell
# Here is how you would import a series of biquad filter to output channel 3:
$ minidsp output 3 peq all import filename.txt
PEQ 0: Applied imported filter: biquad1
PEQ 1: Applied imported filter: biquad2
PEQ 2: Applied imported filter: biquad3
...

# If you were to select a single peq, only one filter would have been imported:
$ minidsp output 3 peq 1 import filename.txt
PEQ 0: Applied imported filter: biquad1
Warning: Some filters were not imported because they didn't fit (try using `all`)
```

### Running multiple commands at once
For the purposes of organizing configurations, a file can be created with commands to run sequentially. It's an easy way to recall a certain preset without changing the devie config preset.

Lines are using the same format at the command line, without the `minidsp` command. 

Example:
```
# Comments are allowed and skipped
# So are empty lines

mute on
config 3
input 0 peq all bypass off
output 0 peq all bypass off
gain -- -30
mute off
```

The command list can be ran using  `minidsp -f ./file.txt`


### udev
In order to run as a non-privileged user under Linux, you may have to add a udev rule for this specific device. Under `/etc/udev/rules.d`, create a file named `99-minidsp.rules` containing:

```
# MiniDSP 2x4HD
ATTR{idVendor}=="2752", ATTR{idProduct}=="0011", MODE="660", GROUP="plugdev"
```

Then reload using:

```
sudo udevadm control --reload-rules
```