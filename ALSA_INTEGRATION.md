# ALSA Mixer Integration for MiniDSP

This document describes the ALSA mixer integration added to minidsp-rs, which provides bidirectional volume synchronization between MiniDSP hardware and the Linux system mixer.

## Overview

MiniDSP devices expose master volume (-127 dB to 0 dB) **ONLY** through the minidsp-rs daemon's proprietary USB protocol. The USB audio interface provides audio streaming but does **NOT** expose writable volume controls natively through ALSA.

The ALSA integration creates a bridge that:
- Exposes MiniDSP master volume through an ALSA control (softvol or virtual control)
- Synchronizes bidirectionally: changes in ALSA update MiniDSP hardware, and vice versa
- Enables system-wide volume control integration

This means you can control your MiniDSP volume using:
- Standard Linux audio controls (e.g., `alsamixer`, GNOME/KDE volume controls)
- Media keys on your keyboard
- Any application that controls ALSA mixer volumes

All changes are synchronized with the MiniDSP hardware in real-time.

## Features

- **Two-way synchronization**: Changes on either MiniDSP or ALSA are synced to the other
- **Virtual control creation**: Creates a dedicated "MiniDSP" control in ALSA (with fallback to existing controls)
- **Automatic on Linux**: Enabled by default on Linux systems (conditional compilation)
- **Fully configurable**: Control name, sync interval, and behavior can be customized
- **Graceful fallback**: Uses existing playback controls (Master/PCM) if virtual creation fails

## Architecture

### Volume Control Architecture

**MiniDSP Hardware**:
- Master volume: -127 dB to 0 dB (controlled via proprietary USB protocol)
- USB audio interface: Provides audio streaming only
- No native writable ALSA volume controls

**minidsp-rs Daemon Integration**:
- Exposes master volume through HTTP/WebSocket API
- Creates ALSA integration using softvol plugin or virtual control
- Provides bidirectional sync: ALSA ↔ MiniDSP hardware

**Important**: Main gain is a separate DSP parameter not exposed through the USB interface. Only master volume is available and controlled via the daemon.

### Components Added

1. **`daemon/src/alsa_mixer.rs`** - Main module containing:
   - `AlsaMixerManager`: Main struct for ALSA mixer control
   - `sync_task()`: Background task for bidirectional synchronization
   - Volume conversion functions (dB ↔ percentage)

2. **`daemon/src/alsa_ctl_ffi.rs`** - FFI module for virtual control creation:
   - `VirtualControl::create()`: Creates virtual ALSA control elements using ALSA C API
   - Direct FFI to alsa-sys for functionality not exposed in safe Rust alsa crate

3. **`daemon/src/config.rs`** - Configuration structure:
   - `AlsaMixer`: Configuration for ALSA integration (enabled, card_name, control_name, etc.)

4. **Modified `daemon/src/main.rs`**:
   - Added ALSA mixer initialization in `App::start()`
   - Loads configuration and passes to AlsaMixerManager
   - Spawns sync task on Linux systems

5. **Modified `daemon/Cargo.toml`**:
   - Added `alsa = "0.9.0"` dependency for Linux targets
   - Added `alsa-sys = "0.3.0"` for FFI access to control creation APIs

### How It Works

```
┌─────────────────┐         ┌──────────────────┐         ┌─────────────┐
│  ALSA Mixer     │◄───────►│  Sync Task       │◄───────►│  MiniDSP    │
│  (System Audio) │         │  (Poll every     │         │  Hardware   │
│                 │         │   500ms)         │         │             │
└─────────────────┘         └──────────────────┘         └─────────────┘
        ▲                            │                           │
        │                            │                           │
        └────────────────────────────┴───────────────────────────┘
                   Both directions synced
```

The sync task:
1. Polls every 500ms to check for changes
2. Detects volume/mute changes on either side
3. Syncs the change to the other side
4. Tracks last known state to avoid update loops

### Volume Conversion

MiniDSP uses a dB scale (-127 dB to 0 dB), which is converted to ALSA's centibel format (hundredths of dB):

- **MiniDSP range**: -127.0 dB to 0.0 dB (floating point)
- **ALSA range**: -12700 to 0 centibels (integer, 1 cb = 0.01 dB)
- **Conversion**: `dB * 100 = centibels`

When ALSA controls support native dB values, the conversion is direct and accurate. For controls without dB support, a fallback percentage-based conversion is used.

## Volume Control Types

### Master Volume (Available)
- **Range**: -127 dB to 0 dB
- **Access**: minidsp-rs daemon HTTP API, ALSA sync
- **API Endpoints**:
  - `POST /devices/0/volume/up`
  - `POST /devices/0/volume/down`
  - `GET /devices/0` (returns current volume in master.volume)

### Main Gain (Not Exposed)
- Main gain is an internal DSP parameter
- NOT available through USB interface
- NOT synced to ALSA
- Use master volume for system-wide volume control

## Usage

### Building

The ALSA integration requires ALSA development libraries on Linux:

```bash
# Install ALSA development libraries
sudo apt-get install libasound2-dev  # Debian/Ubuntu
sudo dnf install alsa-lib-devel      # Fedora
sudo pacman -S alsa-lib              # Arch Linux

# Build the daemon
cd daemon
cargo build --release --bin minidspd
```

### Running

Simply start the daemon as usual:

```bash
./target/release/minidspd
```

On Linux, you should see log messages indicating ALSA initialization:

```
INFO ALSA mixer initialized on card 'default' with control 'MiniDSP'
INFO ALSA mixer initialized successfully
INFO ALSA mixer bidirectional sync enabled
```

### Testing

1. **Change MiniDSP volume via HTTP API**:
   ```bash
   curl -X POST http://localhost:5380/devices/0/volume/up
   ```
   → System volume should increase

2. **Change system volume**:
   ```bash
   amixer set Master 50%
   ```
   → MiniDSP hardware volume should change

3. **Use desktop volume controls**:
   - Press volume up/down keys
   - Use GNOME/KDE volume applet
   → MiniDSP hardware should follow

## Configuration

### Daemon Configuration File

Location: `~/.config/minidsp/config.toml` or `/etc/minidsp/config.toml`

#### ALSA Mixer Section

```toml
[alsa_mixer]
# Enable ALSA mixer integration (Linux only)
enabled = true

# ALSA card for mixer control (where the control lives)
# - "default": Use system default card (may not support softvol)
# - "hw:0": Use first hardware card (recommended for softvol)
# - "hw:1": Use second hardware card
card_name = "hw:0"

# Name of the ALSA control to create/use
# This control syncs bidirectionally with MiniDSP hardware
control_name = "Digital"

# Polling interval in milliseconds
# Lower = more responsive, higher = less CPU usage
sync_interval_ms = 100

# Attempt to create virtual ALSA control element
# If true: Try virtual control, fallback to softvol
# If false: Always use softvol configuration
use_virtual_control = true

# Audio output device (where audio is routed)
# Can be different from card_name
output_device = "hw:0"
```

#### Configuration Field Descriptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `true` | Enable ALSA integration on Linux |
| `card_name` | string | `"default"` | ALSA card for mixer control. Use `aplay -l` to list cards |
| `control_name` | string | `"Digital"` | Name of ALSA control that syncs with MiniDSP |
| `sync_interval_ms` | integer | `100` | Sync polling interval (50-500ms recommended) |
| `use_virtual_control` | bool | `true` | Try creating virtual control (requires permissions) |
| `output_device` | string | `"hw:0"` | ALSA device for audio output |

#### Example Configurations

**Minimal (uses all defaults)**:
```toml
[alsa_mixer]
enabled = true
```

**Separate control and output**:
```toml
[alsa_mixer]
enabled = true
card_name = "hw:0"           # Control lives on card 0
control_name = "Digital"      # Creates "Digital" control
output_device = "hw:1"        # Audio plays through card 1
```

**Multiple MiniDSP devices**:
```toml
[alsa_mixer]
enabled = true
control_name = "MiniDSP 1"    # First device

# Note: Multiple device support requires code changes
# Currently syncs with device index 0 only
```

**Low CPU usage**:
```toml
[alsa_mixer]
sync_interval_ms = 500        # Sync every 500ms instead of 100ms
```

### Separate Audio Output and Volume Control

The ALSA integration allows decoupling audio output from volume control, enabling you to play audio through one device while controlling volume via MiniDSP hardware.

**Use Case**: Play audio through hw:0 (standard sound card) while controlling volume via MiniDSP hardware

```toml
[alsa_mixer]
enabled = true
control_name = "Digital"          # ALSA control that syncs with MiniDSP
output_device = "hw:0"            # Audio plays through hw:0
```

**Application usage**:
```bash
# Program uses hw:0 for audio, "Digital" for volume
program -o hw:0 -V Digital
```

**How it works**:
1. Softvol plugin creates "Digital" control on card 0
2. Audio routes to hw:0 (standard sound card)
3. Daemon syncs "Digital" control ↔ MiniDSP hardware volume
4. Changing "Digital" control updates MiniDSP hardware
5. Changing MiniDSP hardware updates "Digital" control

This allows using a different audio device while maintaining MiniDSP volume control integration.

**Important Note**: The MiniDSP USB audio interface provides audio streaming but does not expose writable volume controls natively. Master volume is ONLY accessible through the minidsp-rs daemon's proprietary USB protocol. The softvol/virtual control approach creates a writable ALSA control that syncs bidirectionally with MiniDSP hardware volume.

### ALSA Softvol Configuration

When `use_virtual_control` fails or is disabled, the daemon creates softvol configuration.

#### Generated Configuration

Location: `~/.asoundrc` (per-user) or `/etc/asound.conf` (system-wide)

**Example generated config**:
```
# MiniDSP Software Volume Control for 2x4HD
# This creates a virtual volume control on card 0
# Audio routes to hw:0 while volume syncs with MiniDSP hardware
# Generated by minidsp-rs daemon

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

#### Softvol Parameters

- **type**: `softvol` - ALSA software volume plugin
- **slave.pcm**: Target device for audio playback
- **control.name**: Name of created control (matches `control_name` in daemon config)
- **control.card**: Card number where control is created (always 0 for softvol)
- **min_dB**: Minimum volume (-127 dB = silence)
- **max_dB**: Maximum volume (0 dB = unity gain)
- **resolution**: Volume steps (256 = 0.5 dB per step)

#### Using the Softvol PCM

Applications can use the softvol PCM directly:

```bash
# Play audio through softvol
aplay -D minidsp_1_softvol file.wav

# Set default output (add to ~/.asoundrc)
pcm.!default {
    type plug
    slave.pcm "minidsp_1_softvol"
}
```

#### Activation

After daemon creates configuration:

```bash
# Reload ALSA configuration
sudo alsactl init

# Or restart ALSA service
sudo systemctl restart alsa-restore

# Or restart applications using ALSA
```

### Troubleshooting Configuration

**Control not found**:
```bash
# List available controls
amixer -c 0 scontrols

# Check if softvol config exists
grep -A5 "minidsp.*softvol" ~/.asoundrc

# Verify card number
aplay -l
```

**Permission denied creating virtual control**:
- Add user to `audio` group: `sudo usermod -a -G audio $USER`
- Fallback: Set `use_virtual_control = false` to use softvol

**Audio routing issues**:
- Verify output device exists: `aplay -L`
- Test playback: `aplay -D hw:0 test.wav`
- Check daemon logs for ALSA errors

## Conditional Compilation

The ALSA integration is only compiled on Linux:

```rust
#[cfg(target_os = "linux")]
// ALSA-specific code here
```

On non-Linux platforms:
- Stub implementations are provided
- No ALSA dependencies are included
- Build and runtime behavior are unchanged

## Troubleshooting

### "Failed to initialize ALSA mixer"

**Cause**: ALSA libraries not installed or no audio card detected.

**Solution**:
- Install ALSA development libraries
- Check `aplay -l` to verify audio devices exist
- Try specifying a specific card: modify `AlsaMixerManager::new()` call in main.rs

### "No suitable playback control found"

**Cause**: No volume controls found on the ALSA card.

**Solution**: Check available controls with `amixer scontrols`

### Volume not syncing

**Cause**: MiniDSP device not connected or not responding.

**Solution**:
- Check daemon logs for connection errors
- Verify device is detected: `curl http://localhost:5380/devices`
- Check device index (currently hardcoded to 0)

## Implementation Notes

### Why Virtual Control Creation?

Creating a dedicated virtual control provides several benefits:
1. **Independent control**: MiniDSP volume doesn't interfere with system master volume
2. **Clear identification**: Shows with device name (e.g., "DDRC-24") in mixers and volume controls
3. **Multiple devices**: Can create separate controls for multiple MiniDSP devices (each with its own product name)
4. **Proper representation**: Accurately reflects the MiniDSP hardware state

However, virtual control creation requires:
- ALSA control element API access (via FFI)
- Proper permissions (usually requires audio group membership)

If creation fails, the fallback to existing controls (Master/PCM) works seamlessly.

### Why Polling Instead of Events?

ALSA does support event-driven notifications, but:
1. MiniDSP doesn't send unsolicited volume updates
2. Polling simplifies the design
3. 100ms interval (10 polls/sec) is very responsive with minimal overhead

### Why Native dB Instead of Percentage?

The implementation uses ALSA's native dB API when available:
1. MiniDSP natively operates in dB (-127 to 0 dB)
2. ALSA controls can report dB ranges directly
3. Direct dB-to-dB conversion avoids precision loss
4. Fallback to percentage mapping when dB not supported

### Thread Safety

- All state is protected by `Arc<Mutex<>>`
- Sync task runs independently
- No blocking operations in critical sections

## Future Enhancements

Possible improvements:

1. **Per-device ALSA controls** when multiple MiniDSP devices are connected
   - Currently uses first device (index 0)
   - Could create "MiniDSP 0", "MiniDSP 1", etc.

2. **Event-driven updates** instead of polling
   - Use ALSA event notifications
   - Would reduce CPU usage further

3. **Logarithmic volume curve** option
   - More natural perceived loudness progression
   - Configurable via config file

4. **Persistent virtual controls** via ALSA UCM
   - Keep virtual controls across daemon restarts
   - Requires ALSA Use Case Manager integration

5. **Per-channel controls**
   - Separate L/R volume controls
   - Currently uses mono (both channels set to same value)

## Files Changed

- `daemon/Cargo.toml` - Added ALSA dependencies (alsa, alsa-sys)
- `daemon/src/config.rs` - Added AlsaMixer configuration struct
- `daemon/src/main.rs` - Integration initialization with config loading
- `daemon/src/alsa_mixer.rs` - Main module for volume synchronization
- `daemon/src/alsa_ctl_ffi.rs` - FFI module for virtual control creation (new)

## License

Same as minidsp-rs (Apache-2.0)
