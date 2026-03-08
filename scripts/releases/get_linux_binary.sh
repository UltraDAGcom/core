#!/bin/bash
# Build Linux binary on VPS and download it locally
set -e

VPS="johanmichel@84.247.10.2"
VERSION="0.1.0"
RELEASE_DIR="releases/$VERSION"

echo "🔨 Building Linux binary on VPS..."
echo ""

# Build on VPS (you'll need to enter password)
ssh -t $VPS << 'ENDSSH'
# Install dependencies if needed
if ! command -v pkg-config &> /dev/null; then
    echo "Installing build dependencies..."
    sudo apt-get update
    sudo apt-get install -y pkg-config libssl-dev build-essential
fi

# Build
cd ~/ultradag-build
source $HOME/.cargo/env
echo "Building release binary (this will take a few minutes)..."
cargo build --release -p ultradag-node

echo ""
echo "✅ Build complete!"
ls -lh target/release/ultradag-node
ENDSSH

echo ""
echo "📥 Downloading Linux binary to local releases folder..."

# Create release directory if it doesn't exist
mkdir -p "$RELEASE_DIR"

# Download the binary
scp "$VPS:~/ultradag-build/target/release/ultradag-node" "$RELEASE_DIR/ultradag-node-linux-x86_64"
chmod +x "$RELEASE_DIR/ultradag-node-linux-x86_64"

# Create tarball and checksum
cd "$RELEASE_DIR"
tar -czf ultradag-node-linux-x86_64.tar.gz ultradag-node-linux-x86_64
shasum -a 256 ultradag-node-linux-x86_64.tar.gz > ultradag-node-linux-x86_64.tar.gz.sha256
cd - > /dev/null

SIZE=$(du -h "$RELEASE_DIR/ultradag-node-linux-x86_64" | cut -f1)

echo ""
echo "✅ Linux binary ready: $SIZE"
echo "   Location: $RELEASE_DIR/ultradag-node-linux-x86_64.tar.gz"
echo "   Checksum: $RELEASE_DIR/ultradag-node-linux-x86_64.tar.gz.sha256"
echo ""
echo "📦 Both binaries ready for release:"
ls -lh "$RELEASE_DIR"/*.tar.gz
echo ""
echo "Next step: ./scripts/publish_release.sh 0.1.0"
