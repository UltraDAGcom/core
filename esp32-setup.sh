#!/bin/bash

# ESP32 UltraDAG Setup Script
echo "Setting up ESP32 development environment for UltraDAG..."

# Install Rust ESP32 target
rustup target add xtensa-esp32-espidf

# Install ESP32 tools
cargo install espflash
cargo install espmonitor
cargo install ldproxy

# Install ESP-IDF (if not already installed)
if ! command -v esp-idf &> /dev/null; then
    echo "Installing ESP-IDF..."
    cargo install espup
    espup install
fi

echo "ESP32 development environment ready!"
echo "Connect your ESP32 and run: cargo run --target xtensa-esp32-espidf"
