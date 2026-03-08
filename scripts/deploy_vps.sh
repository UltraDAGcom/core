#!/bin/bash
# Deploy UltraDAG node to VPS
# Usage: ./deploy_vps.sh

set -e

VPS_HOST="84.247.10.2"
VPS_USER="johanmichel"
VPS_TARGET="$VPS_USER@$VPS_HOST"
NODE_DIR="/home/$VPS_USER/ultradag"
BINARY="target/release/ultradag-node"

echo "🚀 Deploying UltraDAG to VPS: $VPS_HOST"
echo "=========================================="
echo ""

# 1. Build release binary
echo "📦 Building release binary..."
cargo build --release -p ultradag-node

if [ ! -f "$BINARY" ]; then
    echo "❌ Binary not found at $BINARY"
    exit 1
fi

BINARY_SIZE=$(du -h "$BINARY" | cut -f1)
echo "✅ Binary built: $BINARY_SIZE"
echo ""

# 2. Create deployment package
echo "📦 Creating deployment package..."
DEPLOY_DIR="deploy_$(date +%s)"
mkdir -p "$DEPLOY_DIR"

cp "$BINARY" "$DEPLOY_DIR/"
cp scripts/monitor.sh "$DEPLOY_DIR/" 2>/dev/null || true

# Create systemd service file
cat > "$DEPLOY_DIR/ultradag.service" << 'EOF'
[Unit]
Description=UltraDAG Node
After=network.target

[Service]
Type=simple
User=johanmichel
WorkingDirectory=/home/johanmichel/ultradag
ExecStart=/home/johanmichel/ultradag/ultradag-node \
    --validator /home/johanmichel/ultradag/validator.key \
    --seed https://ultradag-node-1.fly.dev \
    --seed https://ultradag-node-2.fly.dev \
    --seed https://ultradag-node-3.fly.dev \
    --seed https://ultradag-node-4.fly.dev \
    --rpc-port 10333 \
    --round-duration-ms 5000
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# Create setup script
cat > "$DEPLOY_DIR/setup.sh" << 'EOF'
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
EOF

chmod +x "$DEPLOY_DIR/setup.sh"

echo "✅ Deployment package created"
echo ""

# 3. Copy to VPS
echo "📤 Copying to VPS..."
ssh "$VPS_TARGET" "mkdir -p $NODE_DIR"
scp -r "$DEPLOY_DIR"/* "$VPS_TARGET:$NODE_DIR/"

if [ $? -ne 0 ]; then
    echo "❌ Failed to copy files to VPS"
    exit 1
fi

echo "✅ Files copied to VPS"
echo ""

# 4. Run setup
echo "🔧 Running setup on VPS..."
ssh "$VPS_TARGET" "cd $NODE_DIR && bash setup.sh"

if [ $? -ne 0 ]; then
    echo "❌ Setup failed"
    exit 1
fi

echo ""
echo "✅ Deployment complete!"
echo ""

# 5. Check status
echo "📊 Checking node status..."
sleep 5

ssh "$VPS_TARGET" "sudo systemctl status ultradag --no-pager" || true

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "VPS Node Deployed Successfully!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Node address: http://84.247.10.2:10333"
echo ""
echo "Useful commands (run on VPS):"
echo "  Status:  sudo systemctl status ultradag"
echo "  Logs:    sudo journalctl -u ultradag -f"
echo "  Restart: sudo systemctl restart ultradag"
echo "  Stop:    sudo systemctl stop ultradag"
echo ""
echo "API endpoints:"
echo "  curl http://84.247.10.2:10333/status"
echo "  curl http://84.247.10.2:10333/round/1000"
echo ""

# Cleanup
rm -rf "$DEPLOY_DIR"
