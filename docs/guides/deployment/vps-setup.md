# VPS Setup Commands

The files have been copied to your VPS. Now SSH into the VPS and run these commands:

```bash
ssh johanmichel@84.247.10.2
```

Then run:

```bash
cd ~/ultradag

# Install systemd service
sudo cp ultradag.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable ultradag
sudo systemctl start ultradag

# Check status
sudo systemctl status ultradag

# View logs
sudo journalctl -u ultradag -f
```

## Test the node

From your local machine:

```bash
curl http://84.247.10.2:10333/status
```

## Useful commands

```bash
# Status
sudo systemctl status ultradag

# Restart
sudo systemctl restart ultradag

# Stop
sudo systemctl stop ultradag

# View logs (follow)
sudo journalctl -u ultradag -f

# View logs (last 100 lines)
sudo journalctl -u ultradag -n 100
```

## Validator key location

The validator key is at: `~/ultradag/validator.key`

**Keep this safe!** This is your validator's private key.
