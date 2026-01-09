#!/bin/bash
#
# Diagnostic script to identify why volume sync isn't working
#

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     MiniDSP Volume Sync Diagnostic Tool                   ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check 1: MiniDSP USB Device
echo -e "${BLUE}[Check 1]${NC} MiniDSP USB Device"
if lsusb | grep -i "2752" > /dev/null; then
    echo -e "${GREEN}✓${NC} MiniDSP USB device detected:"
    lsusb | grep -i "2752"
else
    echo -e "${RED}✗${NC} No MiniDSP USB device found (VID 2752)"
    echo "  Solution: Connect your MiniDSP device via USB"
fi
echo ""

# Check 2: USB Permissions
echo -e "${BLUE}[Check 2]${NC} USB Permissions"
if groups | grep -q "plugdev\|audio"; then
    echo -e "${GREEN}✓${NC} User is in audio/plugdev group"
else
    echo -e "${YELLOW}!${NC} User may need to be in 'audio' or 'plugdev' group"
    echo "  Solution: sudo usermod -a -G audio,plugdev $USER"
    echo "  Then log out and back in"
fi
echo ""

# Check 3: Daemon Binary
echo -e "${BLUE}[Check 3]${NC} Daemon Binary"
if [ -f "target/release/minidspd" ]; then
    echo -e "${GREEN}✓${NC} Daemon binary exists: target/release/minidspd"
    echo "  Size: $(ls -lh target/release/minidspd | awk '{print $5}')"
    echo "  Modified: $(stat -c %y target/release/minidspd | cut -d. -f1)"
else
    echo -e "${RED}✗${NC} Daemon binary not found"
    echo "  Solution: cargo build --release -p minidsp-daemon"
fi
echo ""

# Check 4: ALSA Installation
echo -e "${BLUE}[Check 4]${NC} ALSA Tools"
if command -v amixer &> /dev/null; then
    echo -e "${GREEN}✓${NC} amixer command available"
else
    echo -e "${RED}✗${NC} amixer not found"
    echo "  Solution: sudo apt install alsa-utils"
fi
echo ""

# Check 5: ALSA Controls
echo -e "${BLUE}[Check 5]${NC} ALSA Mixer Controls"
if amixer scontrols | grep -i "digital\|minidsp" > /dev/null; then
    echo -e "${GREEN}✓${NC} Found MiniDSP-related ALSA control:"
    amixer scontrols | grep -i "digital\|minidsp"
else
    echo -e "${YELLOW}!${NC} No 'Digital' or 'MiniDSP' control found"
    echo ""
    echo "Available controls:"
    amixer scontrols | head -10
    echo ""
    echo -e "${YELLOW}Possible issues:${NC}"
    echo "  1. ALSA softvol not configured"
    echo "  2. Need to regenerate ~/.asoundrc"
    echo "  3. Wrong card selected"
fi
echo ""

# Check 6: ALSA Configuration
echo -e "${BLUE}[Check 6]${NC} ALSA Configuration (~/.asoundrc)"
if [ -f ~/.asoundrc ]; then
    echo -e "${GREEN}✓${NC} ~/.asoundrc exists"
    if grep -q "minidsp.*softvol\|resolution 255" ~/.asoundrc; then
        echo -e "${GREEN}✓${NC} Contains minidsp softvol configuration"
        if grep -q "resolution 255" ~/.asoundrc; then
            echo -e "${GREEN}✓${NC} Has correct resolution 255"
        elif grep -q "resolution 254" ~/.asoundrc; then
            echo -e "${YELLOW}!${NC} Has OLD resolution 254 (should be 255)"
            echo "  Solution: Regenerate config or edit manually"
        fi
    else
        echo -e "${YELLOW}!${NC} No minidsp softvol config found in .asoundrc"
    fi
else
    echo -e "${RED}✗${NC} ~/.asoundrc not found"
    echo "  The daemon may need to generate this file"
fi
echo ""

# Check 7: Running Daemon
echo -e "${BLUE}[Check 7]${NC} Running Daemon"
if pgrep -x minidspd > /dev/null; then
    PID=$(pgrep -x minidspd)
    echo -e "${GREEN}✓${NC} Daemon is running (PID: $PID)"

    # Check if it's the right binary
    RUNNING_BINARY=$(readlink -f /proc/$PID/exe 2>/dev/null || echo "unknown")
    echo "  Binary: $RUNNING_BINARY"

    # Check when it was started
    STARTED=$(ps -p $PID -o lstart= 2>/dev/null || echo "unknown")
    echo "  Started: $STARTED"

    echo ""
    echo -e "${YELLOW}Note:${NC} If daemon is running with old code, restart it:"
    echo "  pkill minidspd && target/release/minidspd"
else
    echo -e "${YELLOW}!${NC} No daemon currently running"
    echo "  Start with: target/release/minidspd"
fi
echo ""

# Check 8: minidsp CLI
echo -e "${BLUE}[Check 8]${NC} MiniDSP CLI Communication"
if command -v minidsp &> /dev/null || [ -f "target/release/minidsp" ]; then
    MINIDSP_BIN="target/release/minidsp"
    if ! [ -f "$MINIDSP_BIN" ]; then
        MINIDSP_BIN="minidsp"
    fi

    echo "Testing device communication..."
    if OUTPUT=$($MINIDSP_BIN -v 2>&1); then
        echo -e "${GREEN}✓${NC} Can communicate with MiniDSP device"
        echo "$OUTPUT" | head -5

        # Try to get master volume
        if VOLUME=$($MINIDSP_BIN master-status 2>&1 | grep -i volume); then
            echo -e "${GREEN}✓${NC} Master volume accessible:"
            echo "  $VOLUME"
        fi
    else
        echo -e "${RED}✗${NC} Cannot communicate with device"
        echo "  Error: $OUTPUT"
    fi
else
    echo -e "${YELLOW}!${NC} minidsp CLI not found"
    echo "  Build with: cargo build --release"
fi
echo ""

# Check 9: Kernel Messages
echo -e "${BLUE}[Check 9]${NC} Recent USB/Audio Kernel Messages"
if DMESG=$(dmesg | grep -i "usb\|audio\|minidsp" | tail -5 2>/dev/null); then
    if [ -n "$DMESG" ]; then
        echo "Recent kernel messages:"
        echo "$DMESG"
    else
        echo "No recent USB/audio messages"
    fi
else
    echo -e "${YELLOW}!${NC} Cannot read dmesg (need sudo)"
fi
echo ""

# Summary
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}Summary${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
echo ""

# Count issues
CRITICAL=0
WARNING=0

if ! lsusb | grep -i "2752" > /dev/null; then
    ((CRITICAL++))
    echo -e "${RED}CRITICAL:${NC} No MiniDSP USB device detected"
fi

if ! [ -f "target/release/minidspd" ]; then
    ((CRITICAL++))
    echo -e "${RED}CRITICAL:${NC} Daemon binary not built"
fi

if ! amixer scontrols | grep -i "digital\|minidsp" > /dev/null; then
    ((WARNING++))
    echo -e "${YELLOW}WARNING:${NC} No ALSA softvol control configured"
fi

if grep -q "resolution 254" ~/.asoundrc 2>/dev/null; then
    ((WARNING++))
    echo -e "${YELLOW}WARNING:${NC} Old resolution 254 in .asoundrc (should be 255)"
fi

if [ $CRITICAL -eq 0 ] && [ $WARNING -eq 0 ]; then
    echo -e "${GREEN}✓ All checks passed!${NC}"
    echo ""
    echo "If sync still doesn't work, run the test script with:"
    echo "  ./test_volume_sync.sh"
elif [ $CRITICAL -gt 0 ]; then
    echo ""
    echo -e "${RED}Fix critical issues above, then try again${NC}"
else
    echo ""
    echo -e "${YELLOW}Fix warnings above for best results${NC}"
    echo "You can still try running: ./quick_test.sh"
fi
echo ""
