#!/usr/bin/env bash
set -euo pipefail

REPO="ekhodzitsky/oh-my-kimi"
BINARY="omk"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    arm64|aarch64) ARCH="aarch64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
    linux) TARGET="${ARCH}-unknown-linux-gnu" ;;
    darwin) TARGET="${ARCH}-apple-darwin" ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

echo "Installing omk for ${TARGET}..."

# Try cargo install first if Rust is available
if command -v cargo >/dev/null 2>&1; then
    echo "Rust detected. Installing via cargo..."
    cargo install --git "https://github.com/${REPO}.git"
    echo "✓ Installed via cargo"
    exit 0
fi

# Fallback: download prebuilt binary (placeholder for future releases)
LATEST_URL="https://github.com/${REPO}/releases/latest/download/${BINARY}-${TARGET}.tar.gz"
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$LATEST_URL" -o "${TMP_DIR}/${BINARY}.tar.gz" || true
elif command -v wget >/dev/null 2>&1; then
    wget -q "$LATEST_URL" -O "${TMP_DIR}/${BINARY}.tar.gz" || true
fi

if [ -f "${TMP_DIR}/${BINARY}.tar.gz" ] && [ -s "${TMP_DIR}/${BINARY}.tar.gz" ]; then
    tar -xzf "${TMP_DIR}/${BINARY}.tar.gz" -C "$TMP_DIR"
    chmod +x "${TMP_DIR}/${BINARY}"
    
    # Install to ~/.local/bin or /usr/local/bin
    if [ -w "/usr/local/bin" ]; then
        mv "${TMP_DIR}/${BINARY}" "/usr/local/bin/"
        echo "✓ Installed to /usr/local/bin/${BINARY}"
    else
        mkdir -p "$HOME/.local/bin"
        mv "${TMP_DIR}/${BINARY}" "$HOME/.local/bin/"
        echo "✓ Installed to ~/.local/bin/${BINARY}"
        echo "Add ~/.local/bin to your PATH if not already present."
    fi
else
    echo "Prebuilt binary not available for ${TARGET}."
    echo "Please install Rust and run: cargo install --git https://github.com/${REPO}.git"
    exit 1
fi
