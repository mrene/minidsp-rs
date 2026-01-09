#!/bin/bash

echo "Current MiniDSP volume:"
curl -s http://localhost:5380/devices/0 | jq '.master.volume'

echo
echo "Current ALSA Digital:"
amixer -c 0 get Digital | grep "Front Left:"

echo
echo "--- Changing MiniDSP volume UP ---"
curl -s -X POST http://localhost:5380/devices/0/volume/up | jq '.master.volume'

sleep 0.3

echo
echo "ALSA Digital after change:"
amixer -c 0 get Digital | grep "Front Left:"

echo
echo "--- Changing ALSA Digital to 50% ---"
amixer -c 0 set Digital 50% | grep "Front Left:"

sleep 0.3

echo
echo "MiniDSP volume after ALSA change:"
curl -s http://localhost:5380/devices/0 | jq '.master.volume'

echo
echo "=== Test: Separate Audio Output ==="
echo "Config: output_device = hw:0, control = Digital"

if grep -q "slave.pcm.*hw:0" ~/.asoundrc 2>/dev/null; then
    echo "✓ Softvol configured for hw:0 output"

    # Check control exists
    if amixer -c 0 sget Digital > /dev/null 2>&1; then
        echo "✓ Digital control found"
        echo
        echo "Current Digital control value:"
        amixer -c 0 sget Digital | grep "Front Left:"

        # Test sync
        echo
        echo "--- Testing MiniDSP -> Digital sync ---"
        curl -s -X POST http://localhost:5380/devices/0/volume/up > /dev/null
        sleep 0.3
        echo "Digital control after MiniDSP change:"
        amixer -c 0 sget Digital | grep "Front Left:"

        echo
        echo "--- Testing Digital -> MiniDSP sync ---"
        amixer -c 0 sset Digital 60% > /dev/null 2>&1
        sleep 0.3
        echo "MiniDSP volume after Digital change:"
        curl -s http://localhost:5380/devices/0 | jq '.master.volume'
    else
        echo "✗ Digital control not found"
    fi
else
    echo "⚠ Not using separate output device mode (or using different device)"
fi
