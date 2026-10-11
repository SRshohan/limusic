# Builds crates/server only — `cargo build -p server` resolves just that crate's dependency graph
# (innertube + limusic-core), so the workspace's Tauri-only members (src-tauri, player) and their
# GTK/webkit system deps are never touched here.
FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release -p server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libstdc++6 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/limusic-server /usr/local/bin/limusic-server
# Relative-path db/data (main.rs) land here — mount a volume at /data to persist them.
WORKDIR /data
ENV BIND_ADDR=0.0.0.0:8787
EXPOSE 8787
CMD ["limusic-server"]
