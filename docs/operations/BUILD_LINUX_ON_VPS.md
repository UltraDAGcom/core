# Build Linux Binary on VPS

SSH into your VPS and run these commands:

```bash
ssh johanmichel@84.247.10.2
```

Then run:

```bash
# Install build dependencies
sudo apt-get update
sudo apt-get install -y pkg-config libssl-dev build-essential

# Build the binary
cd ~/ultradag-build
source $HOME/.cargo/env
cargo build --release -p ultradag-node

# Check the binary
ls -lh target/release/ultradag-node
```

## Then from your local machine:

```bash
# Download the Linux binary
scp johanmichel@84.247.10.2:~/ultradag-build/target/release/ultradag-node releases/0.1.0/ultradag-node-linux-x86_64

# Make it executable
chmod +x releases/0.1.0/ultradag-node-linux-x86_64

# Create tarball and checksum
cd releases/0.1.0
tar -czf ultradag-node-linux-x86_64.tar.gz ultradag-node-linux-x86_64
shasum -a 256 ultradag-node-linux-x86_64.tar.gz > ultradag-node-linux-x86_64.tar.gz.sha256
cd ../..

# Verify both binaries exist
ls -lh releases/0.1.0/*.tar.gz

# Publish the release
./scripts/publish_release.sh 0.1.0
```

This will create the official v0.1.0 release with both Linux and macOS binaries!
