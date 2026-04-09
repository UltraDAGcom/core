# UltraDAG ESP32 Deployment

UltraDAG is designed to run on resource-constrained devices like ESP32. This guide shows how to deploy a full UltraDAG node on ESP32 hardware.

## Hardware Requirements

- **ESP32** (ESP32-DevKitC, ESP32-WROOM-32, etc.)
- **Minimum 4MB Flash** (8MB+ recommended)
- **WiFi connectivity**
- **Power supply** (USB or battery)

## Software Requirements

- **Rust 1.70+**
- **ESP-IDF** (v5.0+)
- **ESP32 toolchain**

## Quick Start

### 1. Setup Development Environment

```bash
# Run the setup script
chmod +x esp32-setup.sh
./esp32-setup.sh
```

### 2. Configure WiFi

Edit `crates/ultradag-esp32/src/main.rs`:
```rust
let network_config = NetworkConfig {
    ssid: "YOUR_WIFI_SSID",
    password: "YOUR_WIFI_PASSWORD",
    ultradag_peers: vec![
        "ultradag-node-1.fly.dev:8080".to_string(),
        "ultradag-node-2.fly.dev:8080".to_string(),
    ],
};
```

### 3. Build and Flash

```bash
# Make flash script executable
chmod +x esp32-flash.sh

# Build and flash to ESP32
./esp32-flash.sh
```

### 4. Monitor

```bash
# Monitor serial output
cargo espflash monitor
```

## Features

### UltraDAG Node on ESP32

- **Full consensus** participation
- **Transaction processing**
- **HTTP API server** (port 80)
- **WiFi networking**
- **Memory efficient** (<1MB RAM)
- **Low power** consumption

### API Endpoints

- `GET /status` - Node status
- `POST /tx` - Submit transaction
- `GET /peers` - Connected peers
- `GET /dag` - DAG state

### Memory Usage

- **Flash**: ~800KB
- **RAM**: ~256KB
- **Stack**: 32KB

## Configuration

### Network Settings

```rust
NetworkConfig {
    ssid: "WiFiNetwork",
    password: "WiFiPassword",
    ultradag_peers: vec![
        "node1.example.com:8080",
        "node2.example.com:8080",
    ],
}
```

### Performance Tuning

- **Batch size**: Adjust for memory constraints
- **Sync interval**: Balance responsiveness vs power
- **Cache size**: Optimize for your use case

## Troubleshooting

### Common Issues

1. **Out of memory**
   - Reduce batch sizes
   - Enable `lto = true` in Cargo.toml
   - Use `opt-level = "z"`

2. **WiFi connection fails**
   - Check SSID/password
   - Ensure 2.4GHz network
   - Verify signal strength

3. **Flash too large**
   - Enable `panic = "abort"`
   - Use `strip = true`
   - Remove unused features

### Debug Mode

```bash
# Build with debug symbols
cargo build --target xtensa-esp32-espidf

# Flash debug version
cargo run --target xtensa-esp32-espidf
```

## Performance

### Benchmarks

- **Block time**: ~2 seconds
- **Transaction throughput**: ~10 tx/sec
- **Memory usage**: 256KB peak
- **Power consumption**: ~150mA

### Scaling

Multiple ESP32 nodes can form a network:
- **Mesh networking** support
- **Load balancing** automatic
- **Fault tolerance** built-in

## Development

### Adding Features

1. Modify `crates/ultradag-esp32/src/lib.rs`
2. Update API endpoints in `main.rs`
3. Test with `cargo run`

### Testing

```bash
# Unit tests
cargo test --target xtensa-esp32-espidf

# Integration tests
cargo test --target xtensa-esp32-espidf --features std
```

## Production Deployment

### Security

- **Secure boot** enabled
- **Flash encryption** recommended
- **Network isolation** for sensitive data

### Monitoring

- **Serial logging** for debugging
- **HTTP metrics** for monitoring
- **OTA updates** for remote deployment

### Power Management

- **Deep sleep** support
- **Battery monitoring**
- **Low power modes**

## Support

- **Issues**: GitHub Issues
- **Discussions**: GitHub Discussions
- **Community**: Discord

## License

BUSL-1.1 - See LICENSE file for details.
