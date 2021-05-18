# output [channel] peq

### Importing filters from Room Eq Wizard (REW)
The `minidsp output n peq` and `minidsp input n peq` commands both support importing from a REW-formatted file. If there are less
filters on the device, the remaining PEQs will be cleared.

Here is how you would import a series of biquad filter to output channel 3:
```shell
$ minidsp output 3 peq all import filename.txt
PEQ 0: Applied imported filter: biquad1
PEQ 1: Applied imported filter: biquad2
PEQ 2: Applied imported filter: biquad3
...
```

If you were to select a single peq, only one filter would have been imported:
```shell
$ minidsp output 3 peq 1 import filename.txt
PEQ 0: Applied imported filter: biquad1
Warning: Some filters were not imported because they didn't fit (try using `all`)
```

## Usage
```
{{#include ../../outputs.txt:output_peq_help}}
```
