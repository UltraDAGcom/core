#!/bin/bash

# ESP32 UltraDAG Flash Script
echo "Building and flashing UltraDAG to ESP32..."

# Set target
export ESP_IDF_VERSION=latest
export ESP_IDF_TOOLS_INSTALL_DIR=~/.espressif

# Build for ESP32
echo "Building UltraDAG for ESP32..."
cd crates/ultradag-esp32
cargo build --release --target xtensa-esp32-espidf

# Flash to device
echo "Flashing to ESP32..."
cargo run --release --target xtensa-esp32-espidf

echo "UltraDAG ESP32 node flashed successfully!"
echo "Monitor output with: cargo espflash monitor"
