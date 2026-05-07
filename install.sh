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

install_completions() {
    local bin="$1"
    local shell="$2"
    local dir=""
    
    case "$shell" in
        bash)
            dir="${BASH_COMPLETION_USER_DIR:-$HOME/.local/share/bash-completion/completions}"
            mkdir -p "$dir"
            "$bin" completions bash > "$dir/$BINARY"
            echo "  ✓ Bash completions → $dir/$BINARY"
            ;;
        zsh)
            dir="${ZSH_COMPLETION_DIR:-$HOME/.zsh/completions}"
            mkdir -p "$dir"
            "$bin" completions zsh > "$dir/_$BINARY"
            echo "  ✓ Zsh completions → $dir/_$BINARY"
            ;;
        fish)
            dir="${XDG_CONFIG_HOME:-$HOME/.config}/fish/completions"
            mkdir -p "$dir"
            "$bin" completions fish > "$dir/$BINARY.fish"
            echo "  ✓ Fish completions → $dir/$BINARY.fish"
            ;;
    esac
}

install_manpage() {
    local bin="$1"
    local dir="${XDG_DATA_HOME:-$HOME/.local/share}/man/man1"
    mkdir -p "$dir"
    "$bin" man > "$dir/$BINARY.1"
    echo "  ✓ Man page → $dir/$BINARY.1"
}

echo "Installing omk for ${TARGET}..."

# Try cargo install first if Rust is available
if command -v cargo >/dev/null 2>&1; then
    echo "Rust detected. Installing via cargo..."
    cargo install --git "https://github.com/${REPO}.git"
    
    # Install completions and man page
    OMK_BIN=$(command -v "$BINARY" || echo "$HOME/.cargo/bin/$BINARY")
    if [ -x "$OMK_BIN" ]; then
        echo "Installing shell completions..."
        install_completions "$OMK_BIN" bash
        install_completions "$OMK_BIN" zsh
        install_completions "$OMK_BIN" fish
        install_manpage "$OMK_BIN"
    fi
    
    echo "✓ Installed via cargo"
    exit 0
fi

# Fallback: download prebuilt binary
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
        OMK_BIN="/usr/local/bin/$BINARY"
    else
        mkdir -p "$HOME/.local/bin"
        mv "${TMP_DIR}/${BINARY}" "$HOME/.local/bin/"
        echo "✓ Installed to ~/.local/bin/${BINARY}"
        echo "Add ~/.local/bin to your PATH if not already present."
        OMK_BIN="$HOME/.local/bin/$BINARY"
    fi
    
    # Install completions and man page
    echo "Installing shell completions..."
    install_completions "$OMK_BIN" bash
    install_completions "$OMK_BIN" zsh
    install_completions "$OMK_BIN" fish
    install_manpage "$OMK_BIN"
else
    echo "Prebuilt binary not available for ${TARGET}."
    echo "Please install Rust and run: cargo install --git https://github.com/${REPO}.git"
    exit 1
fi
