# Channel Names

You can assign human-readable names to input and output channels. Names are stored per-device (identified by serial number) in `~/.config/minidsp/names.toml`.

## Setting names

A device must be connected when assigning names:

```bash
minidsp name input 0 left
minidsp name input 1 right
minidsp name output 0 left
minidsp name output 1 right
minidsp name output 2 sub_left
minidsp name output 3 sub_right
```

## Using names

Anywhere a channel index is accepted, you can use a name instead:

```bash
minidsp output left gain -- -10
minidsp output sub_left fir load filter.wav
minidsp input left routing right gain -- -3
```

Numeric indices still work as before:

```bash
minidsp output 0 gain -- -10
```

## Status display

When names are configured, `minidsp status` shows them in the text output:

```
MasterStatus { preset: 0, source: Toslink, volume: Gain(-8.0), mute: false, dirac: false }
Input levels: left: -61.6, right: -57.9
Output levels: left: -67.9, right: -71.6, sub_left: -120.0, sub_right: -120.0
```

JSON output (`-o json`) is unaffected.

## Listing names

Works without a device connected:

```bash
$ minidsp name list
device 902106:
  input  0: left
  input  1: right
  output 0: left, main_l
  output 1: right
  output 2: sub_left
  output 3: sub_right
```

## Aliases

Multiple names can point to the same channel:

```bash
minidsp name output 0 left
minidsp name output 0 main_l
```

The first name appearing in the config file for a given index is used in status display. You can reorder entries in `~/.config/minidsp/names.toml` to change which name is preferred.

## Removing names

```bash
minidsp name remove sub_right
```

## Config file format

```toml
[device.902106.inputs]
left = 0
right = 1

[device.902106.outputs]
left = 0
right = 1
sub_left = 2
sub_right = 3
```

## Notes

- Names cannot be pure numbers (to avoid ambiguity with indices)
- Names are stored per device serial number, so multiple devices can have independent names
- The config file is at `~/.config/minidsp/names.toml`
