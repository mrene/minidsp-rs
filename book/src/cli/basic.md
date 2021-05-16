# Usage
```
{{#include ../outputs.txt:help}}
```

Running the command without any parameters will return a status summary, in this form:

```bash
$ minidsp 
MasterStatus { preset: 0, source: Toslink, volume: Gain(-8.0), mute: false, dirac: false }
Input levels: -61.6, -57.9
Output levels: -67.9, -71.6, -120.0, -120.0
```

### Attention
The changes done through this command will not be visible from the minidsp app, as it cannot read the settings back from the device.
The following settings will be visible after changing them from any source:
- Master Gain
- Master Mute
- Configuration preset
- Active Source
- Dirac Live status

The rest of the settings (filters, delays, routing) will not be reflected in the app.


### Running multiple commands at once
For the purposes of organizing configurations, a file can be created with commands to run sequentially. It's an easy way to recall a certain preset without changing the device config preset.

Lines are using the same format at the command line, without the `minidsp` command. 

Example:
```
# Comments are allowed and skipped
# So are empty lines

mute on
config 3
input 0 peq all bypass off
output 0 peq all bypass off
gain -- -30
mute off
```

The command list can be ran using  `minidsp -f ./file.txt`
