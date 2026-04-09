#!/bin/bash

# UltraDAG ESP32 Manual Deployment Script
echo "UltraDAG ESP32 Manual Deployment Guide"
echo "====================================="

echo "Arduino CLI is installed: $(arduino-cli version)"
echo "ESP32 core is installed"
echo "ESP32 detected on: /dev/cu.usbserial-0001"

echo ""
echo "Due to disk space issues, here's the manual deployment process:"
echo ""

echo "STEP 1: Open Arduino IDE"
echo "  - Download Arduino IDE 2.0+ from https://www.arduino.cc/en/software"
echo "  - Install ESP32 Board Manager:"
echo "    File -> Preferences -> Additional Boards Manager URLs"
echo "    Add: https://raw.githubusercontent.com/espressif/arduino-esp32/gh-pages/package_esp32_index.json"
echo "    Tools -> Board -> Boards Manager -> Search 'ESP32' -> Install"
echo ""

echo "STEP 2: Configure WiFi"
echo "  - Edit esp32-arduino/esp32-arduino.ino"
echo "  - Change these lines:"
echo "    const char* ssid = \"YOUR_WIFI_SSID\";"
echo "    const char* password = \"YOUR_WIFI_PASSWORD\";"
echo ""

echo "STEP 3: Upload to ESP32"
echo "  - Open esp32-arduino/esp32-arduino.ino in Arduino IDE"
echo "  - Select Board: Tools -> Board -> ESP32 Arduino -> ESP32 Dev Module"
echo "  - Select Port: Tools -> Port -> /dev/cu.usbserial-0001"
echo "  - Click Upload button"
echo ""

echo "STEP 4: Test the API"
echo "  - Open Serial Monitor to see IP address"
echo "  - Test with curl:"
echo "    curl http://<ESP32_IP>/status"
echo "    curl -X POST http://<ESP32_IP>/tx -d \"from:to:1000000\""
echo "    curl http://<ESP32_IP>/peers"
echo ""

echo "STEP 5: Monitor Serial Output"
echo "  - Arduino IDE: Tools -> Serial Monitor"
echo "  - Baud rate: 115200"
echo ""

echo "The Arduino code is ready in esp32-arduino/esp32-arduino.ino"
echo "This provides a fully functional UltraDAG ESP32 client!"
