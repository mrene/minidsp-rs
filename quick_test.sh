#!/bin/bash
#
# Quick Manual Test - Run daemon and test volume sync
#

echo "═══════════════════════════════════════════════════"
echo "  Quick MiniDSP Volume Sync Test"
echo "═══════════════════════════════════════════════════"
echo ""

# Stop any existing daemon
echo "[1/4] Stopping existing daemon..."
sudo systemctl stop minidspd 2>/dev/null || true
pkill minidspd 2>/dev/null || true
sleep 1
echo "✓ Done"
echo ""

# Check for device
echo "[2/4] Checking for MiniDSP device..."
if lsusb | grep -i "2752\|minidsp" > /dev/null; then
    echo "✓ MiniDSP device found"
else
    echo "✗ No MiniDSP device found!"
    echo "Please connect your device and try again."
    exit 1
fi
echo ""

# Start daemon with logging
echo "[3/4] Starting daemon with debug logging..."
echo ""
echo "════════════════════════════════════════════════════"
echo "LOGS BELOW - Look for:"
echo "  • 'MiniDSP changed: ... sync: true'"
echo "  • 'Synced ... volume:'"
echo "  • 'Quantizing ALSA'"
echo ""
echo "Press Ctrl+C to stop when done"
echo "════════════════════════════════════════════════════"
echo ""

sleep 2

echo "[4/4] Running daemon (Ctrl+C to stop)..."
echo ""

RUST_LOG=minidsp_daemon=debug,minidsp=info target/release/minidspd
