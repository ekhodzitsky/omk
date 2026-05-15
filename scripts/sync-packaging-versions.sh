#!/usr/bin/env bash
# Sync version + SHA256 from a freshly-released set of artifacts into the
# packaging files we ship in-tree (Homebrew, AUR, flake.nix).
#
# Invoked from .github/workflows/release.yml on a tag push, after binaries
# and `.tar.gz.sha256` checksum files have been built and uploaded as
# workflow artifacts.
#
# Usage:
#   scripts/sync-packaging-versions.sh <version> <artifacts-dir>
#
# Arguments:
#   <version>        bare semver, no `v` prefix (e.g. 0.3.30)
#   <artifacts-dir>  path whose subdirectories contain the per-target
#                    `omk-<version>-<target>.tar.gz.sha256` files
#
# Behaviour:
#   - Homebrew formula: bump `version` + replace SHA256 placeholders for the
#     three release targets.
#   - AUR PKGBUILD: bump `pkgver`. `sha256sums` is left as `SKIP` because
#     AUR consumes the GitHub source tarball, whose hash we cannot derive
#     from binary artifacts.
#   - flake.nix: bump `version`.

set -euo pipefail

if [ "${1:-}" = "" ] || [ "${2:-}" = "" ]; then
    echo "Usage: $0 <version> <artifacts-dir>" >&2
    exit 1
fi

VERSION="$1"
ART_DIR="$2"
REPO_ROOT="$(git rev-parse --show-toplevel)"

if [ ! -d "$ART_DIR" ]; then
    echo "Artifacts directory not found: $ART_DIR" >&2
    exit 1
fi

# Extract the digest from a `<digest>  <filename>` sha256 file. Looks the
# file up by target tuple under any nested artifact subdirectory.
sha_for_target() {
    local target="$1"
    local pattern="omk-${VERSION}-${target}.tar.gz.sha256"
    # `find -print -quit` would short-circuit, but BSD find on macOS doesn't
    # support it; use `head -1` for portability between Linux/macOS runners.
    local sha_file
    sha_file="$(find "$ART_DIR" -type f -name "$pattern" 2>/dev/null | head -1)"
    if [ -z "$sha_file" ]; then
        echo "Checksum file not found for target $target (pattern: $pattern)" >&2
        return 1
    fi
    awk '{print $1; exit}' "$sha_file"
}

echo "Syncing packaging files to version ${VERSION}..."

HOMEBREW_FILE="$REPO_ROOT/homebrew/omk.rb"
AUR_FILE="$REPO_ROOT/aur/PKGBUILD"
FLAKE_FILE="$REPO_ROOT/flake.nix"

# --- Homebrew ---
if [ -f "$HOMEBREW_FILE" ]; then
    SHA_AARCH_MACOS="$(sha_for_target aarch64-apple-darwin)"
    SHA_X86_MACOS="$(sha_for_target x86_64-apple-darwin)"
    SHA_X86_LINUX="$(sha_for_target x86_64-unknown-linux-gnu)"

    # Use sed -i with a backup suffix that works on both GNU and BSD sed.
    SED_INPLACE=(-i.bak)
    sed "${SED_INPLACE[@]}" -E \
        -e "s/^([[:space:]]*version[[:space:]]+\")[^\"]*(\".*)/\1${VERSION}\2/" \
        "$HOMEBREW_FILE"

    # Anchor each sha replacement on a per-target token that is invariant
    # across runs (the per-target URL still names the target tuple, so even
    # after the placeholder is replaced once, the anchor still finds the
    # right block on subsequent runs). The {n; s/...} form advances to the
    # next non-blank `sha256` line within the same url+sha block.
    sed "${SED_INPLACE[@]}" -E \
        -e "/url.*aarch64-apple-darwin/,/sha256/{ s|(sha256[[:space:]]+\")[^\"]*(\")|\1${SHA_AARCH_MACOS}\2|; }" \
        -e "/url.*x86_64-apple-darwin/,/sha256/{ s|(sha256[[:space:]]+\")[^\"]*(\")|\1${SHA_X86_MACOS}\2|; }" \
        -e "/url.*x86_64-unknown-linux-gnu/,/sha256/{ s|(sha256[[:space:]]+\")[^\"]*(\")|\1${SHA_X86_LINUX}\2|; }" \
        "$HOMEBREW_FILE"

    rm -f "${HOMEBREW_FILE}.bak"
    echo "  ✓ Homebrew formula -> ${VERSION}"
fi

# --- AUR ---
if [ -f "$AUR_FILE" ]; then
    sed -i.bak -E -e "s/^(pkgver=).*/\1${VERSION}/" "$AUR_FILE"
    rm -f "${AUR_FILE}.bak"
    # NOTE: sha256sums kept as `SKIP` because the AUR source is the GitHub
    # *source* tarball, not the binary artifacts we publish. Maintainers
    # should run `updpkgsums` locally to refresh it for each tag.
    echo "  ✓ AUR PKGBUILD pkgver -> ${VERSION} (sha256sums still SKIP — run updpkgsums)"
fi

# --- flake.nix ---
if [ -f "$FLAKE_FILE" ]; then
    sed -i.bak -E -e "s/(version = \")[^\"]*(\";)/\1${VERSION}\2/" "$FLAKE_FILE"
    rm -f "${FLAKE_FILE}.bak"
    echo "  ✓ flake.nix version -> ${VERSION}"
fi

echo "Done."
