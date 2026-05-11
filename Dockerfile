# Build stage
FROM rust:1.95.0-slim-bookworm AS builder
ENV RUSTUP_TOOLCHAIN=stable
WORKDIR /usr/src/omk
COPY . .
RUN cargo +stable build --release --features server

# Runtime stage
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
