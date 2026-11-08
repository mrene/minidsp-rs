#!/usr/bin/env bash
# Captures the USB HID traffic between miniDSP Device Console and the device, on macOS.
#
#   ./capture.sh "/Applications/<Device Console>.app" [output.log]
#
# The first run makes a copy of the app at ~/DeviceConsole-capture.app and re-signs the copy
# ad hoc, which removes the hardened runtime that blocks Frida. The installed app is not changed.
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "usage: $0 /path/to/DeviceConsole.app [output.log]" >&2
  exit 1
fi

APP="$1"
OUT="${2:-capture-$(date +%Y%m%d-%H%M%S).log}"
COPY="$HOME/DeviceConsole-capture.app"
HERE="$(cd "$(dirname "$0")" && pwd)"

command -v frida >/dev/null || { echo "frida not found: pipx install frida-tools" >&2; exit 1; }

if [ ! -d "$COPY" ]; then
  echo "Copying $APP to $COPY and re-signing the copy…"
  cp -R "$APP" "$COPY"
  xattr -cr "$COPY"
  codesign --force --deep --sign - "$COPY"
fi

EXE="$(defaults read "$COPY/Contents/Info" CFBundleExecutable)"
echo "Capturing to $OUT"
echo "In the Frida prompt, label each action before doing it, e.g.:  mark(\"set OUT 1 gain to -6\")"
echo "Quit Device Console (or type 'exit') to stop."
exec frida -f "$COPY/Contents/MacOS/$EXE" -l "$HERE/hid-trace.js" -o "$OUT"
