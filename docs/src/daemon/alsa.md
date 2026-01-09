# ALSA Integration

The daemon provides bidirectional volume synchronization between MiniDSP hardware and Linux ALSA system mixer (Linux only).

## Overview

MiniDSP devices expose master volume **only** through the minidsp-rs daemon's proprietary protocol. The USB audio interface provides audio streaming but does **not** expose writable volume controls natively.

The ALSA integration creates a bridge that allows you to control MiniDSP volume using:
- Desktop volume controls (GNOME/KDE volume applets)
- Media keys on your keyboard
- Any application that controls ALSA mixer volumes

All changes are synchronized with the MiniDSP hardware in real-time.

## Quick Start

### 1. Enable ALSA Integration

Add to your config file (`~/.config/minidsp/config.toml`):

```toml
[alsa_mixer]
enabled = true
```

### 2. Start the Daemon

```bash
minidspd --config ~/.config/minidsp/config.toml
```

You should see:
```
INFO ALSA mixer initialized successfully
INFO ALSA mixer bidirectional sync enabled
```

### 3. Test It

```bash
# Change volume via ALSA
amixer -c 0 sset Digital 60%

# Or use your desktop volume controls
# Changes will sync to MiniDSP hardware
```

## Configuration

### Basic Configuration

```toml
[alsa_mixer]
enabled = true              # Enable integration
card_name = "hw:0"          # ALSA card for control
control_name = "Digital"    # Name of ALSA control
sync_interval_ms = 100      # Sync every 100ms
use_virtual_control = true  # Try virtual control
output_device = "hw:0"      # Audio output device
```

### Configuration Options

| Option | Default | Description |
|--------|---------|-------------|
| `enabled` | `true` | Enable ALSA integration |
| `card_name` | `"default"` | ALSA card for mixer control |
| `control_name` | `"Digital"` | Name of ALSA control |
| `sync_interval_ms` | `100` | Sync polling interval (ms) |
| `use_virtual_control` | `true` | Try creating virtual control |
| `output_device` | `"hw:0"` | Audio output device |

Use `aplay -l` to list available cards.

### Example: Separate Audio and Control

Play audio through one device while controlling volume via MiniDSP:

```toml
[alsa_mixer]
enabled = true
card_name = "hw:0"        # Control on card 0
output_device = "hw:1"    # Audio on card 1
control_name = "Digital"
```

## Master Volume vs Main Gain

**Master Volume** (Available):
- Range: -127 dB to 0 dB
- Accessible via daemon HTTP API
- Synced to ALSA

**Main Gain** (Not Available):
- Internal DSP parameter
- NOT exposed through USB interface
- Use master volume instead

## Troubleshooting

### Control Not Found

Check if the control exists:
```bash
amixer -c 0 scontrols
```

If "Digital" is missing, check daemon logs for creation errors.

### Permission Denied

Add your user to the audio group:
```bash
sudo usermod -a -G audio $USER
# Log out and back in
```

Or use softvol instead:
```toml
[alsa_mixer]
use_virtual_control = false
```

### Volume Not Syncing

1. Check daemon is running: `ps aux | grep minidspd`
2. Check device is connected: `curl http://localhost:5380/devices/0`
3. Check logs for sync errors
4. Verify control exists: `amixer -c 0 sget Digital`

### Audio Routing Issues

Test audio output:
```bash
aplay -D hw:0 test.wav
```

List available devices:
```bash
aplay -L
```

## ALSA Softvol Configuration

The daemon automatically creates `~/.asoundrc` configuration when needed:

```
pcm.minidsp_1_softvol {
    type softvol
    slave.pcm "hw:0"
    control {
        name "Digital"
        card 0
    }
    min_dB -127.0
    max_dB 0.0
    resolution 256
}
```

After creation, reload ALSA:
```bash
sudo alsactl init
```

## Integration with System Audio

### Desktop Environments

**GNOME**: Volume controls automatically detect the "Digital" control

**KDE**: Use System Settings → Audio → Device Profiles to select the control

**XFCE/MATE**: Use pavucontrol or xfce4-mixer to select the control

### Media Keys

Volume up/down keys should automatically control the "Digital" control on most desktop environments.

If not working:
1. Check desktop settings for volume key bindings
2. Verify control is visible: `amixer -c 0 scontrols`
3. Test manually: `amixer -c 0 sset Digital 1+`

## Advanced Usage

### Multiple MiniDSP Devices

Currently, ALSA integration syncs with device index 0 only. Multiple device support requires code changes.

### Custom Sync Interval

Lower for more responsive (higher CPU):
```toml
[alsa_mixer]
sync_interval_ms = 50
```

Higher for less CPU (less responsive):
```toml
[alsa_mixer]
sync_interval_ms = 500
```

### Disable Integration

```toml
[alsa_mixer]
enabled = false
```

## See Also

- [HTTP API](./http.md) - Control volume via HTTP
- [TCP Server](./tcp.md) - Plugin app compatibility
- [ALSA_INTEGRATION.md](../../ALSA_INTEGRATION.md) - Technical details
