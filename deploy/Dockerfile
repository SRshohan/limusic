# Builds crates/server only — `cargo build -p server` resolves just that crate's dependency graph
# (innertube + limusic-core), so the workspace's Tauri-only members (src-tauri, player) and their
# GTK/webkit system deps are never touched here.
FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
# Cache mounts: this crate's dependency tree is heavy (deno_core/V8 download, lindera's dictionary
# fetch, a full rusqlite-bundled compile) and every source change otherwise pays for all of it
# again from zero — there's no persistent `docker compose build` state between runs without this.
# The compiled binary has to be copied out of the cache-mounted target dir before the mount's
# contents vanish at the end of this RUN (only this stage's own non-cached layers survive).
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p server && \
    cp target/release/limusic-server /tmp/limusic-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libstdc++6 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /tmp/limusic-server /usr/local/bin/limusic-server
# Relative-path db/data (main.rs) land here — mount a volume at /data to persist them.
WORKDIR /data
ENV BIND_ADDR=0.0.0.0:8787
EXPOSE 8787
CMD ["limusic-server"]
