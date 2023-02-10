# output [channel] compressor

Dynamic range compression (DRC) or simply compression is an electronic effect unit that reduces the volume of loud sounds or amplifies quiet sounds by narrowing or "compressing" an audio signal's dynamic range

The command allows any number of options to be specified.  Unspecified arguments remain at the last set value.


## Examples

#### Protect subwoofers from applying a wrong BEQ profile or leaving a profile enabled as recommended in the Bass EQ thread
```bash
minidsp output 0 compressor -b on -t 0 -k 50 -a 15 -r 30
```

#### Limit the dynamic range of a tactile transducer or bass shaker.  This would allow them to turn on more often but not be driven too hard.
```bash
minidsp output 3 gain -- 12
minidsp output 3 compressor -b on -t -36 -k 50 -a 15 -r 30
```

## Usage

```
{{#include ../../outputs.txt:output_compressor_help}}
```
