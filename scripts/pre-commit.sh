#!/usr/bin/env bash
set -euo pipefail

echo "Running pre-commit checks..."

cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo deny check
cargo doc --no-deps

echo "✓ All checks passed"
