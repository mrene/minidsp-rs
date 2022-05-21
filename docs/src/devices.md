# Supported devices
Depending on the device, two levels of support are available. `Basic Support` devices support a limited subset of features that are common across devices, whereas `Full Support` devices can be completely controlled.

This software can also control these devices if they are connected to a `WI-DG`.

## Full support
These devices support the full feature set (input, output, routing, peqs, etc.)

### Devices
- miniDSP 2x4HD
- miniDSP Flex
- DDRC-24
- DDRC-88A/D
- miniSHARC series
- miniDSP 2x8/8x8/4x10/10x10
- nanoDIGI 2x8
- SHD series
- C-DSP 8x12

## Basic support
For these devices, only the following settings can be changed:
- Master Gain
- Master Mute
- Input Source
- Active Configuration Presets
- Dirac Live Status

### Devices
- C-DSP 6x8 
- nanoSHARC series
- OpenDRC series 
- nanoAVR HD/HDA

# Adding support for new devices
If you have a device that is on the `Basic Support` tier, you can help adding support by inspecting the commands sent by the plugin application.

TODO: Add guide for adding new devices