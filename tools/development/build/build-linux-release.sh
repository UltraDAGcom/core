#!/bin/bash
# Build Linux binary on VPS and download to releases
set -e

VPS="johanmichel@84.247.10.2"
VERSION="0.1.0"
RELEASE_DIR="releases/$VERSION"

echo "🔨 Building Linux binary on VPS..."
echo ""

# Build on VPS
ssh $VPS << 'ENDSSH'
cd ~/ultradag-build
echo "Building release binary..."
cargo build --release -p ultradag-node
echo "✅ Build complete"
ls -lh target/release/ultradag-node
ENDSSH

echo ""
echo "📥 Downloading Linux binary..."

# Download the binary
mkdir -p "$RELEASE_DIR"
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
echo "Both binaries ready for release!"
ls -lh "$RELEASE_DIR"/*.tar.gz
