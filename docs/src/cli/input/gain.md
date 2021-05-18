# input [channel] gain
Sets the input gain for the given channel

Note: Because most gain values are negative, a prefix of `--` is required to pass values.

## Example

#### Sets input channel 0's gain to -10dB
```bash
minidsp input 0 gain -- -10
```

## Usage
```nocopy
{{#include ../../outputs.txt:input_gain_help}}
```