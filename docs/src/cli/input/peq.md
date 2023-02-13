# input [channel] peq
The `peq` commands supports broadcasting an operation on multiple peqs. If specifying
an index, the command will only affect a single filter.

When using the `import` subcommand, any imported PEQs automatically unbypassed.

## Examples

#### Bypass the first peq
```bash
minidsp input 0 peq 0 bypass on
```

#### Bypass all peqs
```bash
minidsp input 0 peq all bypass on
```

#### Importing filters should use the `all` target if the unused filter should also be cleared.
```bash
minidsp input 0 peq all import ./file.txt
```

## Usage
```
{{#include ../../outputs.txt:input_peq_help}}
```
