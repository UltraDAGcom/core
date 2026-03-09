# Deployment Configurations

This directory contains deployment configurations for UltraDAG nodes.

## Structure

```
deployments/
├── fly/          # Fly.io deployment configs
│   ├── fly.toml
│   ├── fly-node-1.toml
│   ├── fly-node-2.toml
│   ├── fly-node-3.toml
│   └── fly-node-4.toml
└── docker/       # Docker deployment configs (future use)
```

## Fly.io Deployment

The `fly/` directory contains configurations for deploying UltraDAG nodes to Fly.io.

**Deploy a single node:**
```bash
cd deployments/fly
fly deploy -c fly-node-1.toml
```

**Deploy all 4 nodes:**
```bash
cd deployments/fly
for i in 1 2 3 4; do
  fly deploy -c fly-node-$i.toml
done
```

See `docs/operations/FLY_TOKEN.md` for Fly.io authentication setup.

## Docker Deployment

Docker deployment files are in `deployments/docker/`:
- `Dockerfile` - Multi-stage build for UltraDAG node
- `docker-compose.yml` - 4-node local network setup

**Quick start:**
```bash
cd deployments/docker
docker-compose up -d
```

See `docs/DOCKER.md` for detailed Docker usage instructions.
