# output [channel] crossover

Crossovers are implemented as series biquad filters. There are two groups of 4 biquads per channel. Each group can be bypassed individually.

The command follows the same syntax as the `peq` command, for the exception that you have to specify the group index (0 or 1) in addition to the peq index (0, 1, 2, 3)


## Examples

#### Import crossovers in REW format
```bash
minidsp output 0 crossover 0 all import ./file.txt
minidsp output 0 crossover 1 all import ./file2.txt
```

## Usage
```
{{#include ../outputs.txt:output_crossover_help}}
```
