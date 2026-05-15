# syntax=docker/dockerfile:1
#
# Build stage. The MSRV from Cargo.toml is 1.78; we build on a current
# stable tag a few releases ahead to pick up rustc improvements without
# drifting too far from MSRV's verification path.
#
# Reproducibility note: tag-only pin (no digest) is intentional for now.
# Dependabot's `docker` ecosystem (configured in .github/dependabot.yml)
# bumps this tag on upstream releases. When we cut a 1.0, pin by digest
# here AND in the runtime base below.
FROM rust:1.85-slim-bookworm AS builder
WORKDIR /usr/src/omk
COPY . .
RUN cargo build --release --features server

# Runtime stage.
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && useradd --create-home --home-dir /home/omk --shell /usr/sbin/nologin omk \
    && mkdir -p \
        /home/omk/.config/omk \
        /home/omk/.local/state/omk \
        /home/omk/.local/share/omk \
        /home/omk/.cache/omk \
    && chown -R omk:omk /home/omk \
    && chmod 700 \
        /home/omk/.config/omk \
        /home/omk/.local/state/omk \
        /home/omk/.local/share/omk \
        /home/omk/.cache/omk \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/omk/target/release/omk /usr/local/bin/omk
USER omk
WORKDIR /home/omk
ENV HOME=/home/omk \
    XDG_CONFIG_HOME=/home/omk/.config \
    XDG_STATE_HOME=/home/omk/.local/state \
    XDG_DATA_HOME=/home/omk/.local/share \
    XDG_CACHE_HOME=/home/omk/.cache
ENTRYPOINT ["omk"]
