#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKDIR="$(mktemp -d /tmp/omk-killer-demo-XXXXXX)"
ORIGINAL_HOME="${HOME:-}"

cleanup() {
  rm -rf "${WORKDIR}"
}
trap cleanup EXIT

# Keep local toolchain caches from the original home so CI/offline runs do not
# need network access after HOME/XDG isolation.
if [[ -n "${ORIGINAL_HOME}" ]]; then
  export CARGO_HOME="${CARGO_HOME:-${ORIGINAL_HOME}/.cargo}"
  export RUSTUP_HOME="${RUSTUP_HOME:-${ORIGINAL_HOME}/.rustup}"
fi

export HOME="${WORKDIR}/home"
export XDG_STATE_HOME="${WORKDIR}/xdg_state"
export XDG_CONFIG_HOME="${WORKDIR}/xdg_config"
export XDG_CACHE_HOME="${WORKDIR}/xdg_cache"
mkdir -p "${HOME}" "${XDG_STATE_HOME}" "${XDG_CONFIG_HOME}" "${XDG_CACHE_HOME}"

cd "${REPO_ROOT}"

cargo test --test mock_kimi_test test_team_demo_fixture_scripted_outcomes_are_stable -- --nocapture
