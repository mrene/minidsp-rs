# Capturing Device Console's USB traffic (macOS)

These tools record what miniDSP Device Console sends to a miniDSP over USB, and decode it into
readable commands. The main goal is to learn how Device Console **reads** settings back from the
device (EQ, gains, crossovers…), which minidsp-rs can't do yet. It also reveals commands specific
to devices like the Flex HTX.

Nothing here changes the device. Only capture while Device Console does normal reads and settings
changes; don't capture firmware updates, and don't replay unknown commands to the device.

## How it works

`hid-trace.js` is a [Frida](https://frida.re) script. It hooks the macOS IOKit HID functions that
Device Console uses to talk to the device and logs every miniDSP report (USB vendor id `0x2752`)
in the format that `minidsp-devtools decode` reads:

```
# 12.403s set OUT 1 gain to -6        ← a marker you added
Sent: 09138000040000000202            ← sent by Device Console
Recv: 020103                          ← sent back by the device
```

macOS blocks Frida from attaching to apps with the hardened runtime, so `capture.sh` runs a copy of
Device Console that is re-signed without it. System Integrity Protection stays on.

## Steps

1. Install Frida: `pipx install frida-tools` (or `pip3 install frida-tools`).
2. Build the decoder with every device definition:
   ```sh
   cargo build -p minidsp-devtools --features devices
   ```
3. Plug in the miniDSP and quit Device Console if it's running.
4. Start the capture, passing the path of the installed app:
   ```sh
   cd devtools/usb-capture
   ./capture.sh "/Applications/<Device Console>.app" flexhtx-read-all.log
   ```
   Device Console opens. If Frida reports it can't access the process, run the same command
   with `sudo`.
5. Before each action, add a marker in the Frida prompt, then do the action in Device Console:
   ```
   mark("connect and read everything")
   mark("OUT 1 PEQ band 1 gain -3 dB")
   ```
6. Quit Device Console to stop. Decode the capture:
   ```sh
   ../../target/debug/minidsp-devtools decode flexhtx-read-all.log --force-kind flexhtx
   ```
   `--force-kind` adds the device's own names for each address. Use the kind matching your
   device (`flexhtx`, `flex`, `m2x4hd`, …).

## What to capture

Short captures with one change each are far easier to analyse than one long session:

1. **Connect only.** Open Device Console, let it connect and finish reading, then quit. This is the
   most valuable capture: it shows how settings are read back.
2. **One change at a time**, each with a marker: an output gain, one PEQ band's gain, one PEQ
   band's frequency, a crossover type, a routing change, a preset switch.
3. **Switch presets** in Device Console, to see what it re-reads afterwards.

Unknown commands decode as `Unknown { cmd_id: …, payload: … }`. Those are the interesting ones.

## Troubleshooting

- **No `Sent`/`Recv` lines.** Device Console may use a different USB API. Edit `hid-trace.js` and set
  `VENDOR_FILTER = -1` to log every HID device. If there's still nothing, it isn't going through
  IOKit HID at all; let us know which version of Device Console you have.
- **The copied app won't open.** Delete `~/DeviceConsole-capture.app` and run `capture.sh` again;
  it will copy and re-sign the app afresh.
- **Upgraded Device Console?** Delete the copy so the next capture uses the new version.
