# Build stage
FROM rust:1.85-slim-bookworm AS builder
WORKDIR /usr/src/omk
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tmux \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/omk/target/release/omk /usr/local/bin/omk
ENTRYPOINT ["omk"]
