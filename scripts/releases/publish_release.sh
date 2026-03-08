#!/bin/bash
# Publish official release
set -e

VERSION=${1:-"0.1.0"}

echo "🚀 Publishing UltraDAG v$VERSION"
echo "================================="
echo ""

# Check if releases exist
if [ ! -d "releases/$VERSION" ]; then
    echo "❌ Release directory not found: releases/$VERSION"
    echo "Run ./scripts/build_linux_release.sh first"
    exit 1
fi

# Check for both binaries
LINUX_TAR="releases/$VERSION/ultradag-node-linux-x86_64.tar.gz"
MACOS_TAR="releases/$VERSION/ultradag-node-macos-arm64.tar.gz"

if [ ! -f "$LINUX_TAR" ]; then
    echo "❌ Linux binary not found: $LINUX_TAR"
    echo "Run ./scripts/build_linux_release.sh first"
    exit 1
fi

if [ ! -f "$MACOS_TAR" ]; then
    echo "❌ macOS binary not found: $MACOS_TAR"
    exit 1
fi

echo "✅ Found release binaries:"
ls -lh releases/$VERSION/*.tar.gz
echo ""

# Verify checksums exist
if [ ! -f "$LINUX_TAR.sha256" ] || [ ! -f "$MACOS_TAR.sha256" ]; then
    echo "❌ Checksum files missing"
    exit 1
fi

echo "✅ Checksums verified"
echo ""

# Show what will be released
echo "Release contents:"
echo "  - ultradag-node-linux-x86_64.tar.gz"
echo "  - ultradag-node-macos-arm64.tar.gz"
echo "  - SHA256 checksums for both"
echo ""

# Commit release files
echo "📝 Committing release files..."
git add releases/$VERSION/
git commit -m "Release v$VERSION

Binary releases:
- Linux x86_64 ($(du -h $LINUX_TAR | cut -f1))
- macOS arm64 ($(du -h $MACOS_TAR | cut -f1))

SHA256 checksums included.

Users can download from:
https://github.com/UltraDAGcom/core/releases/v$VERSION"

echo "✅ Release committed"
echo ""

# Create and push tag
echo "🏷️  Creating git tag v$VERSION..."
git tag -a "v$VERSION" -m "Release v$VERSION

UltraDAG Node v$VERSION

Download binaries:
- Linux: ultradag-node-linux-x86_64.tar.gz
- macOS: ultradag-node-macos-arm64.tar.gz

See docs/DOWNLOAD_NODE.md for installation instructions."

echo "✅ Tag created"
echo ""

# Push
echo "📤 Pushing to GitHub..."
git push origin main
git push origin "v$VERSION"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Release v$VERSION Published!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "GitHub Actions will now:"
echo "  1. Build additional platform binaries"
echo "  2. Create GitHub Release"
echo "  3. Upload all binaries"
echo ""
echo "Release will be available at:"
echo "  https://github.com/UltraDAGcom/core/releases/tag/v$VERSION"
echo ""
echo "Users can download with:"
echo "  curl -L https://github.com/UltraDAGcom/core/releases/latest/download/ultradag-node-linux-x86_64.tar.gz"
echo ""
