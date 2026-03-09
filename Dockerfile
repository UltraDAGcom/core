# Stage 1 — build
FROM rust:1.92-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release -p ultradag-node

# Stage 2 — runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ultradag-node /usr/local/bin/ultradag-node
COPY tools/operations/utilities/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
COPY config/testnet-validators.txt /etc/ultradag/validators.txt
RUN chmod +x /usr/local/bin/docker-entrypoint.sh && mkdir -p /data /etc/ultradag
EXPOSE 9333 10333
ENTRYPOINT ["docker-entrypoint.sh"]
