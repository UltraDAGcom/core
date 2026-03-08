#!/bin/bash
# Create release binaries for distribution
set -e

VERSION=${1:-"latest"}
RELEASE_DIR="releases/$VERSION"

echo "🚀 Creating UltraDAG Release Binaries"
echo "======================================"
echo "Version: $VERSION"
echo ""

mkdir -p "$RELEASE_DIR"

# Build macOS binary (current platform)
echo "📦 Building macOS binary..."
cargo build --release -p ultradag-node

MACOS_BINARY="target/release/ultradag-node"
if [ ! -f "$MACOS_BINARY" ]; then
    echo "❌ macOS binary not found"
    exit 1
fi

# Determine macOS architecture
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    MACOS_NAME="ultradag-node-macos-arm64"
else
    MACOS_NAME="ultradag-node-macos-x86_64"
fi

cp "$MACOS_BINARY" "$RELEASE_DIR/$MACOS_NAME"
chmod +x "$RELEASE_DIR/$MACOS_NAME"

MACOS_SIZE=$(du -h "$RELEASE_DIR/$MACOS_NAME" | cut -f1)
echo "✅ macOS binary created: $MACOS_SIZE"

# Create tarball
cd "$RELEASE_DIR"
tar -czf "$MACOS_NAME.tar.gz" "$MACOS_NAME"
shasum -a 256 "$MACOS_NAME.tar.gz" > "$MACOS_NAME.tar.gz.sha256"
cd - > /dev/null

echo "✅ macOS release package created"
echo ""

# Instructions for Linux binary
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "macOS Binary Ready!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Location: $RELEASE_DIR/$MACOS_NAME.tar.gz"
echo "Checksum: $RELEASE_DIR/$MACOS_NAME.tar.gz.sha256"
echo ""
echo "To create Linux binary, run on a Linux machine:"
echo "  cargo build --release -p ultradag-node"
echo "  cp target/release/ultradag-node ultradag-node-linux-x86_64"
echo "  tar -czf ultradag-node-linux-x86_64.tar.gz ultradag-node-linux-x86_64"
echo "  sha256sum ultradag-node-linux-x86_64.tar.gz > ultradag-node-linux-x86_64.tar.gz.sha256"
echo ""
echo "Or use GitHub Actions to build all platforms automatically:"
echo "  git tag v$VERSION"
echo "  git push origin v$VERSION"
echo ""
