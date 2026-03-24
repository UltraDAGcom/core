FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

# Download pre-built binary from GitHub Releases
# VARIANT: "" for testnet, "-mainnet" for mainnet (no faucet, emission-only genesis)
# CACHEBUST is set to the git SHA by the deploy script to force re-download
ARG GITHUB_REPO=UltraDAGcom/core
ARG VERSION=latest
ARG VARIANT=""
ARG CACHEBUST=0
RUN echo "cache-bust: ${CACHEBUST} variant: ${VARIANT}" && \
    curl -fL "https://github.com/${GITHUB_REPO}/releases/download/${VERSION}/ultradag-node${VARIANT}-linux-x86_64" \
    -o /usr/local/bin/ultradag-node && \
    chmod +x /usr/local/bin/ultradag-node

COPY tools/operations/utilities/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

RUN chmod +x /usr/local/bin/docker-entrypoint.sh && mkdir -p /data

EXPOSE 9333 10333
ENTRYPOINT ["docker-entrypoint.sh"]
