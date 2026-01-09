#!/bin/bash
#
# MiniDSP Volume Sync Test Script
# Tests bidirectional volume synchronization between ALSA and MiniDSP
#

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║        MiniDSP Volume Sync Test Script                    ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Function to print status messages
status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

error() {
    echo -e "${RED}[✗]${NC} $1"
}

warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

# Check if running as root (needed for some operations)
if [ "$EUID" -eq 0 ]; then
    warning "Running as root - this is OK but not required"
fi

# Step 1: Check daemon binary
echo ""
status "Step 1: Checking daemon binary..."
if [ -f "target/release/minidspd" ]; then
    success "Daemon binary found: target/release/minidspd"
    DAEMON_PATH="target/release/minidspd"
else
    error "Daemon binary not found at target/release/minidspd"
    status "Building daemon..."
    cargo build --release -p minidsp-daemon
    DAEMON_PATH="target/release/minidspd"
fi

# Step 2: Check if daemon is already running
echo ""
status "Step 2: Checking for running daemon..."
if pgrep -x minidspd > /dev/null; then
    warning "Daemon is already running (PID: $(pgrep -x minidspd))"
    echo ""
    read -p "Kill existing daemon and start fresh? [y/N] " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        status "Stopping existing daemon..."
        sudo systemctl stop minidspd 2>/dev/null || true
        pkill -9 minidspd 2>/dev/null || true
        sleep 1
        success "Stopped existing daemon"
    else
        warning "Keeping existing daemon - sync may not work if it's an old version!"
    fi
else
    success "No daemon currently running"
fi

# Step 3: Check for MiniDSP device
echo ""
status "Step 3: Checking for MiniDSP USB device..."
if lsusb | grep -i "2752\|minidsp" > /dev/null; then
    USB_DEVICE=$(lsusb | grep -i "2752\|minidsp")
    success "MiniDSP device found: $USB_DEVICE"
else
    error "No MiniDSP USB device found!"
    error "Please connect your MiniDSP device and try again"
    exit 1
fi

# Step 4: Check ALSA controls
echo ""
status "Step 4: Checking ALSA mixer controls..."

# Try to find MiniDSP-related controls
status "Looking for ALSA controls..."
ALSA_CONTROLS=$(amixer scontrols 2>/dev/null | grep -i "digital\|minidsp\|master\|pcm" || echo "")

if [ -z "$ALSA_CONTROLS" ]; then
    warning "No obvious MiniDSP-related ALSA controls found"
    echo ""
    status "Available ALSA controls:"
    amixer scontrols
else
    success "Found potential ALSA controls:"
    echo "$ALSA_CONTROLS"
fi

# Try to identify the softvol control
echo ""
status "Checking for softvol control..."
if amixer sget Digital 2>/dev/null | grep -q "Playback"; then
    CONTROL_NAME="Digital"
    success "Found 'Digital' control (default softvol name)"
elif amixer sget MiniDSP 2>/dev/null | grep -q "Playback"; then
    CONTROL_NAME="MiniDSP"
    success "Found 'MiniDSP' control"
else
    warning "Could not find 'Digital' or 'MiniDSP' control"
    echo ""
    status "You may need to:"
    echo "  1. Generate ALSA softvol config"
    echo "  2. Or specify a different control_name in config"
    echo ""
    read -p "Enter control name to test (or press Enter to skip): " CONTROL_NAME
    if [ -z "$CONTROL_NAME" ]; then
        warning "Skipping ALSA control tests"
        CONTROL_NAME=""
    fi
fi

# Step 5: Check current volume
if [ -n "$CONTROL_NAME" ]; then
    echo ""
    status "Step 5: Checking current ALSA volume..."
    CURRENT_VOL=$(amixer sget "$CONTROL_NAME" 2>/dev/null || echo "N/A")
    echo "$CURRENT_VOL"
fi

# Step 6: Start daemon with logging
echo ""
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
status "Step 6: Starting daemon with debug logging..."
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
echo ""
status "Daemon will start in 3 seconds..."
status "Press Ctrl+C to stop the daemon when done testing"
echo ""
sleep 3

# Create a temporary log file
LOG_FILE="/tmp/minidsp_test_$(date +%s).log"
status "Logs will be saved to: $LOG_FILE"
echo ""

# Start daemon in background with logging
RUST_LOG=minidsp_daemon=debug,minidsp=info "$DAEMON_PATH" 2>&1 | tee "$LOG_FILE" &
DAEMON_PID=$!

# Wait for daemon to initialize
status "Waiting for daemon to initialize..."
sleep 3

# Check if daemon is still running
if ! kill -0 $DAEMON_PID 2>/dev/null; then
    error "Daemon failed to start! Check logs above."
    exit 1
fi

success "Daemon started (PID: $DAEMON_PID)"
echo ""

# Step 7: Interactive testing
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}     DAEMON IS RUNNING - READY FOR TESTING                 ${NC}"
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
echo ""

if [ -n "$CONTROL_NAME" ]; then
    echo -e "${YELLOW}Test Scenarios:${NC}"
    echo ""
    echo "1. Test ALSA → MiniDSP sync:"
    echo "   ${BLUE}amixer set $CONTROL_NAME -- -20.0dB${NC}"
    echo "   ${BLUE}amixer set $CONTROL_NAME -- -20.5dB${NC}"
    echo "   ${BLUE}amixer set $CONTROL_NAME -- -21.0dB${NC}"
    echo ""
    echo "2. Test quantization (non-0.5 dB values):"
    echo "   ${BLUE}amixer set $CONTROL_NAME -- -15.3dB${NC}"
    echo "   Should quantize to -15.5 dB"
    echo ""
    echo "3. Test MiniDSP → ALSA sync:"
    echo "   Use hardware volume buttons/remote on MiniDSP"
    echo "   Watch logs for sync messages"
    echo ""
else
    echo -e "${YELLOW}ALSA control not configured - can only test MiniDSP → ALSA${NC}"
    echo ""
    echo "Use hardware volume buttons on MiniDSP and watch logs"
    echo ""
fi

echo -e "${YELLOW}What to look for in logs:${NC}"
echo ""
echo "✓ ${GREEN}Successful sync:${NC}"
echo "  'MiniDSP changed: X dB, ALSA: Y dB, diff: Z dB, sync: true'"
echo "  'Synced MiniDSP -> ALSA volume: X dB'"
echo ""
echo "✓ ${GREEN}Quantization:${NC}"
echo "  'Quantizing ALSA -10.3dB -> -10.5dB before sending to MiniDSP'"
echo ""
echo "✗ ${RED}Problem signs:${NC}"
echo "  'diff: 0.50dB, sync: false' (threshold too high - bug!)"
echo "  'Failed to sync volume' (communication error)"
echo "  No sync messages at all (daemon not monitoring)"
echo ""

# Keep daemon running and show filtered logs
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Showing real-time logs (filtered for volume sync)...${NC}"
echo -e "${BLUE}Press Ctrl+C to stop daemon and exit${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
echo ""

# Function to cleanup on exit
cleanup() {
    echo ""
    echo ""
    status "Stopping daemon..."
    kill $DAEMON_PID 2>/dev/null || true
    sleep 1

    echo ""
    echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}Test Summary${NC}"
    echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
    echo ""

    status "Full logs saved to: $LOG_FILE"
    echo ""

    # Check for common issues in logs
    if grep -q "MiniDSP device not available" "$LOG_FILE"; then
        error "Device connectivity issues detected"
    fi

    if grep -q "sync: false" "$LOG_FILE"; then
        warning "Some sync attempts were skipped - check threshold values"
    fi

    if grep -q "Synced.*volume:" "$LOG_FILE"; then
        success "Volume sync messages found - sync is working!"
        SYNC_COUNT=$(grep -c "Synced.*volume:" "$LOG_FILE")
        echo "  Total syncs: $SYNC_COUNT"
    else
        error "No volume sync messages found - sync may not be working"
    fi

    if grep -q "Quantizing" "$LOG_FILE"; then
        success "Quantization working correctly"
    fi

    echo ""
    status "To view full logs: cat $LOG_FILE"
    status "To search logs: grep -i 'sync\|volume' $LOG_FILE"
    echo ""
}

trap cleanup EXIT INT TERM

# Follow the log file with filtering
tail -f "$LOG_FILE" | grep --line-buffered -E "changed:|sync:|Synced|Quantizing|Actual|diff:|volume|ALSA|MiniDSP|ERROR|WARN" | while read line; do
    if echo "$line" | grep -q "Synced"; then
        echo -e "${GREEN}$line${NC}"
    elif echo "$line" | grep -q "Quantizing"; then
        echo -e "${YELLOW}$line${NC}"
    elif echo "$line" | grep -q "sync: true"; then
        echo -e "${GREEN}$line${NC}"
    elif echo "$line" | grep -q "sync: false"; then
        echo -e "${RED}$line${NC}"
    elif echo "$line" | grep -q "ERROR\|Failed"; then
        echo -e "${RED}$line${NC}"
    elif echo "$line" | grep -q "WARN"; then
        echo -e "${YELLOW}$line${NC}"
    else
        echo "$line"
    fi
done
