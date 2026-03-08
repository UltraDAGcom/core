#!/bin/bash
set -e

echo "Setting up UltraDAG node..."

# Create directory
mkdir -p ~/ultradag
cd ~/ultradag

# Make binary executable
chmod +x ultradag-node

# Generate validator key if doesn't exist
if [ ! -f validator.key ]; then
    echo "Generating validator key..."
    # Create a random 32-byte key
    head -c 32 /dev/urandom | base64 > validator.key
    echo "✅ Validator key generated"
else
    echo "✅ Validator key already exists"
fi

# Install systemd service
echo "Installing systemd service..."
sudo cp ultradag.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable ultradag
sudo systemctl restart ultradag

echo ""
echo "✅ UltraDAG node installed and started"
echo ""
echo "Check status: sudo systemctl status ultradag"
echo "View logs: sudo journalctl -u ultradag -f"
echo "Node API: http://84.247.10.2:10333/status"
