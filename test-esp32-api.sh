#!/bin/bash

# Test script for UltraDAG ESP32 API
# Replace ESP32_IP with your device's actual IP

ESP32_IP="192.168.1.100"  # Update this to your ESP32's IP

echo "Testing UltraDAG ESP32 API at $ESP32_IP"
echo "========================================"

# Test 1: Get status
echo "1. Testing /status endpoint..."
curl -s "http://$ESP32_IP/status" | jq . 2>/dev/null || curl -s "http://$ESP32_IP/status"
echo ""

# Test 2: Get peers
echo "2. Testing /peers endpoint..."
curl -s "http://$ESP32_IP/peers" | jq . 2>/dev/null || curl -s "http://$ESP32_IP/peers"
echo ""

# Test 3: Submit transaction
echo "3. Testing /tx endpoint..."
TX_RESPONSE=$(curl -s -X POST "http://$ESP32_IP/tx" -d "1234567890123456789012345678901234567890:0987654321098765432109876543210987654321:1000000")
echo "Response: $TX_RESPONSE" | jq . 2>/dev/null || echo "Response: $TX_RESPONSE"
echo ""

# Test 4: Get status again to see pending transaction
echo "4. Checking status after transaction..."
curl -s "http://$ESP32_IP/status" | jq . 2>/dev/null || curl -s "http://$ESP32_IP/status"
echo ""

echo "========================================"
echo "API tests completed!"
echo ""
echo "If you see connection errors:"
echo "1. Update ESP32_IP variable to your device's IP"
echo "2. Check that ESP32 is connected to the same WiFi network"
echo "3. Verify the ESP32 is running (check serial monitor)"
echo ""
echo "To find your ESP32's IP:"
echo "- Check serial monitor output"
echo "- Look at your router's connected devices"
echo "- Use network scanning tools"
