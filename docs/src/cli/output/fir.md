# output [channel] fir
Importing FIR filters can be done using a wav file. The file's sampling rate MUST match the device's internal rate. 

## Examples
#### Import FIR filter from impulse.wav
```shell
minidsp output 0 fir import ./impulse.wav
```

#### Unbypass fir filter
```shell
minidsp output 0 fir bypass off
```

## Usage
```
{{#include ../../outputs.txt:output_fir_help}}
```
