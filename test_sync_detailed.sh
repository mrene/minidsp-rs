#!/bin/bash

# Stop any running daemon
pkill minidspd
sleep 1

# Start daemon with debug logging
echo "Starting daemon with debug logging..."
RUST_LOG=debug ./target/release/minidspd > /tmp/minidspd_debug.log 2>&1 &
DAEMON_PID=$!
echo "Daemon started (PID: $DAEMON_PID)"

# Wait for initialization
sleep 3

echo
echo "=== Testing MiniDSP -> ALSA sync ==="
echo "Before: ALSA Digital = $(amixer -c 0 get Digital | grep 'Front Left:' | awk '{print $5}')"
echo "Before: MiniDSP vol  = $(curl -s http://localhost:5380/devices/0 | jq -r .master.volume) dB"

curl -s -X POST http://localhost:5380/devices/0/volume/up > /dev/null
sleep 0.3

echo "After:  ALSA Digital = $(amixer -c 0 get Digital | grep 'Front Left:' | awk '{print $5}')"
echo "After:  MiniDSP vol  = $(curl -s http://localhost:5380/devices/0 | jq -r .master.volume) dB"

echo
echo "=== Testing ALSA -> MiniDSP sync ==="
echo "Before: ALSA Digital = $(amixer -c 0 get Digital | grep 'Front Left:' | awk '{print $5}')"
echo "Before: MiniDSP vol  = $(curl -s http://localhost:5380/devices/0 | jq -r .master.volume) dB"

amixer -c 0 set Digital 60% > /dev/null 2>&1
sleep 0.3

echo "After:  ALSA Digital = $(amixer -c 0 get Digital | grep 'Front Left:' | awk '{print $5}')"
echo "After:  MiniDSP vol  = $(curl -s http://localhost:5380/devices/0 | jq -r .master.volume) dB"

echo
echo "=== Recent sync-related logs ==="
grep -i "sync\|minidsp.*alsa\|alsa.*minidsp" /tmp/minidspd_debug.log | tail -10

echo
echo "Daemon is still running. To stop: kill $DAEMON_PID"
echo "View full logs: cat /tmp/minidspd_debug.log"
