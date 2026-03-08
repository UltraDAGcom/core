#!/bin/bash
# Build UltraDAG directly on VPS
set -e

VPS_HOST="84.247.10.2"
VPS_USER="johanmichel"
VPS_TARGET="$VPS_USER@$VPS_HOST"

echo "🚀 Building UltraDAG on VPS"
echo "============================"
echo ""

# 1. Copy source code to VPS
echo "📤 Copying source code to VPS..."
rsync -avz --exclude 'target' --exclude '.git' \
    ./ "$VPS_TARGET:~/ultradag-build/"

echo "✅ Source code copied"
echo ""

# 2. Build on VPS
echo "🔨 Building on VPS (this may take a few minutes)..."
ssh "$VPS_TARGET" << 'ENDSSH'
set -e

# Install Rust if not present
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Install build dependencies
echo "Installing build dependencies..."
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev

# Build
cd ~/ultradag-build
echo "Building release binary..."
cargo build --release -p ultradag-node

# Copy to ultradag directory
mkdir -p ~/ultradag
cp target/release/ultradag-node ~/ultradag/
chmod +x ~/ultradag/ultradag-node

echo "✅ Build complete"
ls -lh ~/ultradag/ultradag-node
ENDSSH

echo ""
echo "✅ Build successful!"
echo ""
echo "Now run on VPS:"
echo "  cd ~/ultradag"
echo "  sudo systemctl restart ultradag"
echo "  sudo systemctl status ultradag"
