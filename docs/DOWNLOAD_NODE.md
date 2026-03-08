# Download UltraDAG Node

Pre-built binaries for Linux and macOS are available for download.

## Quick Start

### Linux (x86_64)

```bash
# Download
curl -L https://github.com/UltraDAGcom/core/releases/latest/download/ultradag-node-linux-x86_64.tar.gz -o ultradag-node.tar.gz

# Verify checksum (optional but recommended)
curl -L https://github.com/UltraDAGcom/core/releases/latest/download/ultradag-node-linux-x86_64.tar.gz.sha256 -o ultradag-node.tar.gz.sha256
sha256sum -c ultradag-node.tar.gz.sha256

# Extract
tar -xzf ultradag-node.tar.gz

# Make executable
chmod +x ultradag-node-linux-x86_64

# Run
./ultradag-node-linux-x86_64 --help
```

### macOS (Intel)

```bash
# Download
curl -L https://github.com/UltraDAGcom/core/releases/latest/download/ultradag-node-macos-x86_64.tar.gz -o ultradag-node.tar.gz

# Verify checksum (optional but recommended)
curl -L https://github.com/UltraDAGcom/core/releases/latest/download/ultradag-node-macos-x86_64.tar.gz.sha256 -o ultradag-node.tar.gz.sha256
shasum -a 256 -c ultradag-node-macos-x86_64.tar.gz.sha256

# Extract
tar -xzf ultradag-node.tar.gz

# Make executable
chmod +x ultradag-node-macos-x86_64

# Run
./ultradag-node-macos-x86_64 --help
```

### macOS (Apple Silicon / M1/M2/M3)

```bash
# Download
curl -L https://github.com/UltraDAGcom/core/releases/latest/download/ultradag-node-macos-arm64.tar.gz -o ultradag-node.tar.gz

# Verify checksum (optional but recommended)
curl -L https://github.com/UltraDAGcom/core/releases/latest/download/ultradag-node-macos-arm64.tar.gz.sha256 -o ultradag-node.tar.gz.sha256
shasum -a 256 -c ultradag-node-macos-arm64.tar.gz.sha256

# Extract
tar -xzf ultradag-node.tar.gz

# Make executable
chmod +x ultradag-node-macos-arm64

# Run
./ultradag-node-macos-arm64 --help
```

## Running a Validator Node

### 1. Generate a Validator Key

```bash
# Create a random 32-byte key
head -c 32 /dev/urandom | base64 > validator.key
```

**Important:** Keep this key safe! This is your validator's private key.

### 2. Run the Node

**Testnet:**
```bash
./ultradag-node-linux-x86_64 \
    --validator validator.key \
    --seed https://ultradag-node-1.fly.dev \
    --seed https://ultradag-node-2.fly.dev \
    --seed https://ultradag-node-3.fly.dev \
    --seed https://ultradag-node-4.fly.dev \
    --rpc-port 10333 \
    --round-duration-ms 5000
```

**Mainnet (when available):**
```bash
./ultradag-node-linux-x86_64 \
    --validator validator.key \
    --seed https://mainnet-seed-1.ultradag.com \
    --seed https://mainnet-seed-2.ultradag.com \
    --rpc-port 10333 \
    --round-duration-ms 30000
```

### 3. Check Node Status

```bash
curl http://localhost:10333/status
```

## Running as a Service (Linux)

Create `/etc/systemd/system/ultradag.service`:

```ini
[Unit]
Description=UltraDAG Node
After=network.target

[Service]
Type=simple
User=YOUR_USERNAME
WorkingDirectory=/home/YOUR_USERNAME/ultradag
ExecStart=/home/YOUR_USERNAME/ultradag/ultradag-node-linux-x86_64 \
    --validator /home/YOUR_USERNAME/ultradag/validator.key \
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
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable ultradag
sudo systemctl start ultradag
sudo systemctl status ultradag
```

View logs:

```bash
sudo journalctl -u ultradag -f
```

## Running as a Service (macOS)

Create `~/Library/LaunchAgents/com.ultradag.node.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.ultradag.node</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Users/YOUR_USERNAME/ultradag/ultradag-node-macos-arm64</string>
        <string>--validator</string>
        <string>/Users/YOUR_USERNAME/ultradag/validator.key</string>
        <string>--seed</string>
        <string>https://ultradag-node-1.fly.dev</string>
        <string>--seed</string>
        <string>https://ultradag-node-2.fly.dev</string>
        <string>--seed</string>
        <string>https://ultradag-node-3.fly.dev</string>
        <string>--seed</string>
        <string>https://ultradag-node-4.fly.dev</string>
        <string>--rpc-port</string>
        <string>10333</string>
        <string>--round-duration-ms</string>
        <string>5000</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/Users/YOUR_USERNAME/ultradag/ultradag.log</string>
    <key>StandardErrorPath</key>
    <string>/Users/YOUR_USERNAME/ultradag/ultradag.error.log</string>
</dict>
</plist>
```

Load and start:

```bash
launchctl load ~/Library/LaunchAgents/com.ultradag.node.plist
launchctl start com.ultradag.node
```

View logs:

```bash
tail -f ~/ultradag/ultradag.log
```

## Building from Source

If you prefer to build from source:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone repository
git clone https://github.com/UltraDAGcom/core.git
cd core

# Build
cargo build --release -p ultradag-node

# Binary will be at: target/release/ultradag-node
```

## Firewall Configuration

Make sure port 10333 is accessible:

**Linux (ufw):**
```bash
sudo ufw allow 10333/tcp
```

**Linux (iptables):**
```bash
sudo iptables -A INPUT -p tcp --dport 10333 -j ACCEPT
```

**macOS:**
```bash
# System Preferences > Security & Privacy > Firewall > Firewall Options
# Add ultradag-node and allow incoming connections
```

## Troubleshooting

### Node won't start

Check logs:
```bash
# Linux
sudo journalctl -u ultradag -n 100

# macOS
tail -100 ~/ultradag/ultradag.error.log
```

### Can't connect to peers

1. Check firewall allows port 10333
2. Verify seed nodes are reachable:
   ```bash
   curl https://ultradag-node-1.fly.dev/status
   ```

### High memory usage

This is normal during initial sync. Memory usage will stabilize after syncing.

## Support

- GitHub Issues: https://github.com/UltraDAGcom/core/issues
- Documentation: https://github.com/UltraDAGcom/core
- Bug Bounty: See `security/bug-bounty/PROGRAM.md`
