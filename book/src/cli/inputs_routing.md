# Routing
Each output matrix entry has to be enabled in order for audio to be routed. The gain can then be set (in dB) for each entry.

Input and output indices always start at 0.

## Example
#### Route input channel 0 to output channel 0, boost gain by 6dB
```bash
minidsp input 0 routing 0 enable on
minidsp input 0 routing 0 gain 6
```

## Usage
```
{{#include ../outputs.txt:input_routing_help}}
```
