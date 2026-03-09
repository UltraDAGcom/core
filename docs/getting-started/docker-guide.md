# Docker Deployment Guide

UltraDAG provides Docker images for easy cross-platform deployment on Linux, macOS, and Windows.

## Quick Start

### Single Node

```bash
docker run -p 9333:9333 -p 10333:10333 \
  -v ultradag-data:/data \
  ghcr.io/ultradagcom/core:latest \
  --port 9333 --validate
```

Access RPC at `http://localhost:10333/status`

### Multi-Node Local Network

```bash
# Clone repository
git clone https://github.com/UltraDAGcom/core.git
cd core/deployments/docker

# Start 4-node network
docker-compose up -d

# View logs
docker-compose logs -f

# Check node status
curl http://localhost:10333/status  # node1
curl http://localhost:10334/status  # node2
curl http://localhost:10335/status  # node3
curl http://localhost:10336/status  # node4

# Stop network
docker-compose down
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `NODE_PORT` | 9333 | P2P listen port |
| `VALIDATE` | false | Enable block production |
| `ROUND_MS` | 5000 | Round duration in milliseconds |

### Command-Line Arguments

All `ultradag-node` CLI arguments are supported:

```bash
docker run ghcr.io/ultradagcom/core:latest --help
```

**Common options:**
- `--port <PORT>` — P2P port (default: 9333)
- `--rpc-port <PORT>` — HTTP RPC port (default: P2P + 1000)
- `--seed <ADDR:PORT>` — Bootstrap peer address
- `--validate` — Enable validator mode
- `--round-ms <MS>` — Round duration (default: 5000)
- `--data-dir <PATH>` — Data directory (default: /data)
- `--validators <N>` — Fixed validator count
- `--validator-key <FILE>` — Permissioned validator allowlist

## Persistent Storage

### Data Volume

```bash
# Create named volume
docker volume create ultradag-data

# Run with persistent storage
docker run -v ultradag-data:/data \
  ghcr.io/ultradagcom/core:latest
```

### Backup State

```bash
# Backup
docker run --rm -v ultradag-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/ultradag-backup.tar.gz -C /data .

# Restore
docker run --rm -v ultradag-data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/ultradag-backup.tar.gz -C /data
```

## Joining Testnet

```bash
docker run -d \
  --name ultradag-testnet \
  -p 9333:9333 -p 10333:10333 \
  -v ultradag-testnet:/data \
  ghcr.io/ultradagcom/core:latest \
  --port 9333 \
  --validate \
  --seed ultradag-node-1.fly.dev:9333 \
  --seed ultradag-node-2.fly.dev:9333
```

## Building from Source

```bash
# Build image
docker build -t ultradag-node .

# Run locally built image
docker run -p 9333:9333 -p 10333:10333 ultradag-node
```

### Multi-Platform Build

```bash
# Build for multiple architectures
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t ultradag-node:latest \
  --load .
```

## Docker Compose Examples

### Development Network (4 nodes)

See `docker-compose.yml` in the repository root.

```bash
docker-compose up -d
```

### Custom Configuration

```yaml
version: '3.8'
services:
  validator:
    image: ghcr.io/ultradagcom/core:latest
    ports:
      - "9333:9333"
      - "10333:10333"
    volumes:
      - ./validator-data:/data
      - ./validators.txt:/etc/ultradag/validators.txt
    command:
      - --port=9333
      - --validate
      - --validator-key=/etc/ultradag/validators.txt
      - --validators=4
      - --round-ms=5000
```

## Health Checks

```bash
# Check if node is running
docker exec ultradag-node-1 curl -f http://localhost:10333/status

# View validator key
docker exec ultradag-node-1 cat /root/.ultradag/node/validator.key
```

## Troubleshooting

### Container won't start

```bash
# Check logs
docker logs ultradag-node-1

# Interactive shell
docker exec -it ultradag-node-1 /bin/bash
```

### Port conflicts

```bash
# Use different host ports
docker run -p 19333:9333 -p 20333:10333 \
  ghcr.io/ultradagcom/core:latest
```

### Reset state

```bash
# Stop container
docker stop ultradag-node-1

# Remove volume
docker volume rm ultradag-data

# Restart
docker start ultradag-node-1
```

## Production Deployment

### Docker Swarm

```yaml
version: '3.8'
services:
  validator:
    image: ghcr.io/ultradagcom/core:latest
    deploy:
      replicas: 1
      restart_policy:
        condition: on-failure
        max_attempts: 3
    ports:
      - "9333:9333"
      - "10333:10333"
    volumes:
      - validator-data:/data
    command: ["--port", "9333", "--validate"]

volumes:
  validator-data:
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: ultradag-validator
spec:
  serviceName: ultradag
  replicas: 1
  selector:
    matchLabels:
      app: ultradag
  template:
    metadata:
      labels:
        app: ultradag
    spec:
      containers:
      - name: ultradag
        image: ghcr.io/ultradagcom/core:latest
        ports:
        - containerPort: 9333
          name: p2p
        - containerPort: 10333
          name: rpc
        volumeMounts:
        - name: data
          mountPath: /data
        args:
        - --port=9333
        - --validate
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 10Gi
```

## Security Considerations

- **Validator keys**: Stored in `/data/validator.key` — backup securely
- **Network exposure**: Only expose RPC port (10333) if needed
- **Resource limits**: Set memory/CPU limits in production
- **Updates**: Use specific version tags, not `latest`

## Platform Support

Docker images are built for:
- **linux/amd64** — Intel/AMD x86_64
- **linux/arm64** — ARM64 (Apple Silicon, Raspberry Pi 4+)

Tested on:
- ✅ Ubuntu 22.04+
- ✅ macOS 12+ (Intel & Apple Silicon)
- ✅ Windows 10/11 with WSL2
- ✅ Raspberry Pi 4 (8GB RAM recommended)

## Resources

- **Docker Hub**: https://github.com/UltraDAGcom/core/pkgs/container/core
- **Source**: https://github.com/UltraDAGcom/core
- **Issues**: https://github.com/UltraDAGcom/core/issues
