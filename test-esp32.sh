#!/bin/bash

# ESP32 UltraDAG Test Script
echo "Testing UltraDAG ESP32 deployment..."

# Check if ESP32 is connected
echo "Checking for ESP32 device..."
if ! command -v espflash &> /dev/null; then
    echo "ERROR: espflash not installed. Run ./esp32-setup.sh first"
    exit 1
fi

# List connected devices
echo "Connected ESP32 devices:"
espflash board-info 2>/dev/null || echo "No ESP32 devices found"

# Build the project
echo "Building UltraDAG ESP32 client..."
cd crates/ultradag-esp32

# Try to build
if cargo build --release --target xtensa-esp32-espidf; then
    echo "Build successful!"
    
    # Flash if device is connected
    echo "Attempting to flash to ESP32..."
    if cargo run --release --target xtensa-esp32-espidf; then
        echo "Flash successful!"
        echo ""
        echo "=== UltraDAG ESP32 Client Running ==="
        echo "Connect to the ESP32's WiFi network or use its IP to test:"
        echo "  curl http://<ESP32_IP>/status"
        echo "  curl -X POST http://<ESP32_IP>/tx -d 'from:to:100'"
        echo "  curl http://<ESP32_IP>/peers"
        echo ""
        echo "Monitor serial output with:"
        echo "  cargo espflash monitor"
    else
        echo "Flash failed. Check ESP32 connection."
        echo "Make sure:"
        echo "  1. ESP32 is connected via USB"
        echo "  2. Boot mode is correct (hold BOOT button, press RESET)"
        echo "  3. Drivers are installed"
    fi
else
    echo "Build failed. Check dependencies:"
    echo "  1. Run ./esp32-setup.sh"
    echo "  2. Check ESP-IDF installation"
    echo "  3. Verify Rust target is installed: rustup target add xtensa-esp32-espidf"
fi

echo ""
echo "For troubleshooting, see ESP32_DEPLOYMENT.md"
