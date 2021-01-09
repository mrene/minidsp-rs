# MiniDSP Command-line interface

This provides a command line interface to control MiniDSP devices. 
It's a complete rewrite from `node-minidsp` and aims to support multiple devices. Feel free to open an issue if you have access to other hardware!


## Installation

```shell
cargo install minidsp
```

## Usage
```shell
USAGE:
    minidsp [OPTIONS] [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -f <file>          Read commands to run from the given filename
        --tcp <tcp>    The target address of the server component
        --usb <usb>...    The USB vendor and product id (2752:0011 for the 2x4HD)

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
    source    Set the active input sourcee
```

#### Running without arguments will print information about the current state:
```shell
$ minidsp 
MasterStatus { preset: 0, source: Toslink, volume: Gain(-36.5), mute: false }
Input levels: -131.4, -131.4
Output levels: -168.0, -168.0, -120.0, -120.0
```

Commands are organized in different categories:
### Commands changing global state
```shell
# Set input source to toslink
minidsp source toslink

# Set master volume to -30dB
minidsp gain -- -30

# Activate the 2nd configuration setting (indexing starts at 0)
minidsp config 1

```

### Input channel configuration
> minidsp input <input-index> <SUBCOMMAND>
> For more details, look at `minidsp input --help`

#### Examples
```shell
# Sets input channel 0's gain to -10dB
minidsp input 0 gain -- -10

# Mute input channel 0
minidsp input 0 mute on

# Route input channel 0 to output channel 0, boost gain by 6dB
minidsp input 0 routing 0 enable on
minidsp input 0 routing 0 gain 6

# Bypass the first PEQ on input channel 1
minidsp input 1 peq 0 bypass on
````

### Output channel configuration
```shell
>  minidsp output <output-index> <SUBCOMMAND>
> For more details, look at `minidsp output --help`
```
#### Examples
```shell
# Set the delay on output channel 0 to 0.10ms
minidsp output 0 delay 0.10

# Mute output channel 1 
minidsp output 1 mute on

# Invert output channel 1's phase
minidsp output 1 invert on

# Bypass the first PEQ on output channel 1
minidsp input 1 peq 0 bypass on

```

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
For the purposes of organizing configurations, a file can be created with commands to run sequentially.

Files are using the same format at the command line, without the `minidsp` command.

Example:
```
config 3
input 0 peq all bypass off
output 0 peq all bypass off
mute off
```

> minidsp -f ./file.txt

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