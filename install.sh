#!/usr/bin/env bash
set -euo pipefail

REPO="ekhodzitsky/omk"
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
# Resolve the latest release tag via GitHub API so the asset filename — which
# embeds the version (e.g. `omk-0.3.30-x86_64-apple-darwin.tar.gz`) — can be
# constructed without guessing. The old `releases/latest/download/<asset>`
# shortcut required a non-versioned asset name we never publish.
fetch() {
    local url="$1" out="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$out"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$out"
    else
        echo "Neither curl nor wget is available; cannot download prebuilt binary." >&2
        return 1
    fi
}

TMP_DIR=$(mktemp -d)
# Quote $TMP_DIR so a tempdir containing spaces / shell metacharacters does
# not break cleanup. Also trap signals beyond EXIT so a Ctrl-C mid-download
# does not leak the directory.
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM HUP

LATEST_JSON="${TMP_DIR}/latest.json"
if ! fetch "https://api.github.com/repos/${REPO}/releases/latest" "$LATEST_JSON"; then
    echo "Could not query GitHub for the latest release of ${REPO}."
    echo "Please install Rust and run: cargo install --git https://github.com/${REPO}.git"
    exit 1
fi

# Extract `"tag_name": "v0.3.30"` without a JSON parser dependency.
TAG=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$LATEST_JSON" | head -1)
if [ -z "${TAG:-}" ]; then
    echo "Could not parse a release tag from the GitHub API response."
    exit 1
fi
VERSION="${TAG#v}"
ASSET="${BINARY}-${VERSION}-${TARGET}.tar.gz"
SHA_ASSET="${ASSET}.sha256"
BASE_URL="https://github.com/${REPO}/releases/download/${TAG}"

if ! fetch "${BASE_URL}/${ASSET}" "${TMP_DIR}/${ASSET}"; then
    echo "Prebuilt binary not available for ${TARGET} at ${BASE_URL}/${ASSET}."
    echo "Please install Rust and run: cargo install --git https://github.com/${REPO}.git"
    exit 1
fi

# SHA256 verification is mandatory. Without it, a MITM on the CDN or a
# compromised release artifact would land arbitrary code on the host.
if ! fetch "${BASE_URL}/${SHA_ASSET}" "${TMP_DIR}/${SHA_ASSET}"; then
    echo "SHA256 checksum not found at ${BASE_URL}/${SHA_ASSET}."
    echo "Refusing to install an unverified binary. Please re-run with cargo: \\"
    echo "  cargo install --git https://github.com/${REPO}.git"
    exit 1
fi

(
    cd "$TMP_DIR"
    # The sha256 file is `<digest>  <filename>` and the filename it references
    # is the bare asset basename, matching what we downloaded into TMP_DIR.
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum -c "${SHA_ASSET}" >/dev/null
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 -c "${SHA_ASSET}" >/dev/null
    else
        echo "Neither sha256sum nor shasum is available; cannot verify the download." >&2
        exit 1
    fi
)
echo "✓ SHA256 verified for ${ASSET}"

# Tarball is flat (members at the root, no versioned subdirectory). Extract
# into a sandbox dir and pluck `${BINARY}` out by name.
EXTRACT_DIR="${TMP_DIR}/extract"
mkdir -p "$EXTRACT_DIR"
tar --no-same-owner -xzf "${TMP_DIR}/${ASSET}" -C "$EXTRACT_DIR"
if [ ! -f "${EXTRACT_DIR}/${BINARY}" ]; then
    echo "Tarball did not contain '${BINARY}' at the top level; refusing to install."
    exit 1
fi
chmod +x "${EXTRACT_DIR}/${BINARY}"

# Install to ~/.local/bin or /usr/local/bin
if [ -w "/usr/local/bin" ]; then
    mv "${EXTRACT_DIR}/${BINARY}" "/usr/local/bin/"
    echo "✓ Installed to /usr/local/bin/${BINARY}"
    OMK_BIN="/usr/local/bin/$BINARY"
else
    mkdir -p "$HOME/.local/bin"
    mv "${EXTRACT_DIR}/${BINARY}" "$HOME/.local/bin/"
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
