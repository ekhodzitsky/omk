#!/usr/bin/env bash
#
# North Star Demo Script for oh-my-kimi
#
# Demonstrates the first lazy `omk goal` user path:
#   omk setup
#   omk goal run "<goal>" --until-ready
#   omk goal replay latest
#   omk goal proof latest --format json
# The default CI-safe path is:
#   NORTH_STAR_DRY_RUN=1 bash scripts/north_star_demo.sh
# Dry-run mode forces a mock Kimi runtime and isolates HOME/XDG state so the
# demo never spends real tokens or mutates the user's Kimi configuration.

set -euo pipefail

RESET='\033[0m'; GREEN='\033[0;32m'; RED='\033[0;31m'
YELLOW='\033[0;33m'; BLUE='\033[0;34m'; BOLD='\033[1m'

pass() { echo -e "${GREEN}[ok]${RESET} $*"; }
fail() { echo -e "${RED}[fail]${RESET} $*"; }
info() { echo -e "${BLUE}[info]${RESET} $*"; }
header() { echo -e "\n${BOLD}$*${RESET}\n${BOLD}$(printf '=%.0s' $(seq 1 ${#1}))${RESET}"; }

EXIT_CODE=0
DEMOS_DIR=""
USE_MOCK=false
MOCK_KIMI_PATH=""
OMK_CMD=""
REAL_HOME="${HOME:-}"
GOAL_TEXT="Build a tiny local Rust CLI fixture until it has proof-backed setup, terminal progress, and a clear readiness result. Keep the run offline, deterministic, and free of new dependencies."

cleanup() {
    if [[ -n "${DEMOS_DIR}" && -d "${DEMOS_DIR}" ]]; then
        rm -rf "${DEMOS_DIR}"
        pass "Cleaned up temp directory"
        DEMOS_DIR=""
    fi
}

trap cleanup EXIT

is_truthy() {
    [[ "${1:-}" == "1" || "${1:-}" == "true" || "${1:-}" == "yes" ]]
}

find_omk() {
    if command -v omk >/dev/null 2>&1; then
        echo "omk"
        return
    fi

    local repo_root
    repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

    if [[ -x "${repo_root}/target/release/omk" ]]; then
        echo "${repo_root}/target/release/omk"
        return
    fi

    if [[ -x "${repo_root}/target/debug/omk" ]]; then
        echo "${repo_root}/target/debug/omk"
        return
    fi

    echo "cargo run --quiet --release --bin omk --"
}

supports_goal_runtime() {
    ${OMK_CMD} goal --help >/dev/null 2>&1
}

check_kimi() {
    if is_truthy "${NORTH_STAR_DRY_RUN:-}"; then
        USE_MOCK=true
        if [[ -n "${MOCK_KIMI:-}" ]] && ! is_truthy "${MOCK_KIMI}"; then
            if [[ ! -x "${MOCK_KIMI}" ]]; then
                fail "MOCK_KIMI is set but not executable: ${MOCK_KIMI}"
                exit 1
            fi
            MOCK_KIMI_PATH="${MOCK_KIMI}"
        fi
        return
    fi

    if [[ -n "${MOCK_KIMI:-}" ]]; then
        USE_MOCK=true
        if ! is_truthy "${MOCK_KIMI}"; then
            if [[ ! -x "${MOCK_KIMI}" ]]; then
                fail "MOCK_KIMI is set but not executable: ${MOCK_KIMI}"
                exit 1
            fi
            MOCK_KIMI_PATH="${MOCK_KIMI}"
        fi
        return
    fi

    if command -v kimi >/dev/null 2>&1; then
        USE_MOCK=false
        return
    fi

    if command -v mock-kimi >/dev/null 2>&1; then
        USE_MOCK=true
        return
    fi

    local repo_root
    repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    if [[ -x "${repo_root}/target/debug/mock-kimi" || -x "${repo_root}/target/release/mock-kimi" ]]; then
        USE_MOCK=true
        return
    fi

    fail "Neither 'kimi' nor 'mock-kimi' found."
    info "  Install Kimi CLI: https://www.kimi.com/code/docs"
    info "  Or run with NORTH_STAR_DRY_RUN=1 for a fully mocked demo"
    exit 1
}

write_mock_kimi() {
    cat > "${DEMOS_DIR}/mock-kimi-wire" <<'PYEOF'
#!/usr/bin/env python3
import argparse
import json
import os
import sys

parser = argparse.ArgumentParser()
parser.add_argument("--wire", action="store_true")
parser.add_argument("--work-dir")
parser.add_argument("--version", action="store_true")
args, _ = parser.parse_known_args()

if args.version:
    print("kimi version 0.1.0-mock")
    sys.exit(0)

if not args.wire:
    print("mock-kimi: wire mode only for this demo")
    sys.exit(0)

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    if msg.get("method") == "initialize":
        print(json.dumps({"jsonrpc": "2.0", "id": msg["id"], "result": {"protocol_version": "1.9"}}), flush=True)
    elif msg.get("method") == "prompt":
        if os.environ.get("MOCK_KIMI_WRITE_FILE"):
            with open(os.environ["MOCK_KIMI_WRITE_FILE"], "w", encoding="utf-8") as handle:
                handle.write(os.environ.get("MOCK_KIMI_WRITE_BODY", "mock kimi project mutation\n"))
        print(json.dumps({"jsonrpc": "2.0", "id": msg["id"], "result": {"status": "ok"}}), flush=True)
        print(json.dumps({"jsonrpc": "2.0", "method": "event", "params": {"type": "ContentPart", "payload": {"type": "text", "text": "mock goal worker finished"}}}), flush=True)
        print(json.dumps({"jsonrpc": "2.0", "method": "event", "params": {"type": "TurnEnd", "payload": {}}}), flush=True)
        sys.exit(0)
PYEOF
    chmod +x "${DEMOS_DIR}/mock-kimi-wire"
}

json_field() {
    local expr="$1"
    python3 -c "
import json
import sys

raw = sys.stdin.read()
decoder = json.JSONDecoder()
data = None

for index, char in enumerate(raw):
    if char != '{':
        continue
    try:
        data = decoder.raw_decode(raw[index:])[0]
        break
    except json.JSONDecodeError:
        pass

if data is None:
    sys.exit(1)

print(${expr})
" 2>/dev/null || true
}

prepare_project() {
    DEMOS_DIR="$(mktemp -d /tmp/omk-north-star-XXXXXX)"
    info "Temp project: ${DEMOS_DIR}"

    if ${USE_MOCK}; then
        if [[ -z "${CARGO_HOME:-}" && -n "${REAL_HOME}" && -d "${REAL_HOME}/.cargo" ]]; then
            export CARGO_HOME="${REAL_HOME}/.cargo"
        fi
        if [[ -z "${RUSTUP_HOME:-}" && -n "${REAL_HOME}" && -d "${REAL_HOME}/.rustup" ]]; then
            export RUSTUP_HOME="${REAL_HOME}/.rustup"
        fi
        export HOME="${DEMOS_DIR}/home"
        export XDG_STATE_HOME="${DEMOS_DIR}/xdg_state"
        export XDG_CONFIG_HOME="${DEMOS_DIR}/xdg_config"
        export XDG_CACHE_HOME="${DEMOS_DIR}/xdg_cache"
        mkdir -p "${HOME}" "${XDG_STATE_HOME}" "${XDG_CONFIG_HOME}" "${XDG_CACHE_HOME}"
        info "Using isolated HOME/XDG paths for MOCK mode"
    fi

    mkdir -p "${DEMOS_DIR}/src" "${DEMOS_DIR}/.omk"
    cat > "${DEMOS_DIR}/Cargo.toml" <<'EOF'
[package]
name = "north-star-fixture"
version = "0.1.0"
edition = "2021"
EOF
    cat > "${DEMOS_DIR}/src/lib.rs" <<'EOF'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 5);
    }
}
EOF
    cat > "${DEMOS_DIR}/.omk/gates.toml" <<'EOF'
[[gates]]
name = "tests"
command = "cargo"
args = ["test"]
required = true
EOF
    pass "Created temp Rust project with an intentional failing test"

    if ${USE_MOCK}; then
        perl -0pi -e 's/assert_eq!\(add\(2, 2\), 5\);/assert_eq!(add(2, 2), 4);/' "${DEMOS_DIR}/src/lib.rs"
        pass "MOCK mode repaired the fixture deterministically"
        if [[ -z "${MOCK_KIMI_PATH}" ]]; then
            write_mock_kimi
            MOCK_KIMI_PATH="${DEMOS_DIR}/mock-kimi-wire"
        fi
        export MOCK_KIMI="${MOCK_KIMI_PATH}"
        info "MOCK_KIMI=${MOCK_KIMI_PATH}"
    fi

    if command -v git >/dev/null 2>&1; then
        git -C "${DEMOS_DIR}" init >/dev/null 2>&1 || true
        git -C "${DEMOS_DIR}" config user.email omk@example.com >/dev/null 2>&1 || true
        git -C "${DEMOS_DIR}" config user.name "OMK Demo" >/dev/null 2>&1 || true
        git -C "${DEMOS_DIR}" add . >/dev/null 2>&1 || true
        git -C "${DEMOS_DIR}" commit -m "baseline" >/dev/null 2>&1 || true
    fi
}

run_goal_demo() {
    header "Step 2: omk setup"
    local start_dir
    start_dir="$(pwd)"
    cd "${DEMOS_DIR}"
    if ${OMK_CMD} setup >/dev/null 2>&1; then
        pass "omk setup completed"
    else
        fail "omk setup failed"
        cd "${start_dir}" >/dev/null
        return 1
    fi

    header "Step 3: omk goal run --until-ready"
    info "Running one lazy command; follow-up commands below only inspect persisted evidence."
    local goal_output
    if goal_output="$(${OMK_CMD} goal run "${GOAL_TEXT}" --until-ready --budget-time 30m --budget-tokens 200000 --max-agents 1 2>&1)"; then
        pass "omk goal run --until-ready completed"
        echo "${goal_output}"
    else
        echo "${goal_output}"
        fail "omk goal run --until-ready failed"
        cd "${start_dir}" >/dev/null
        return 1
    fi

    header "Step 4: terminal progress"
    local show_json replay_text status phase until_ready state_path
    if ! show_json="$(${OMK_CMD} goal show latest --json 2>&1)"; then
        echo "${show_json}"
        fail "omk goal show latest --json failed"
        cd "${start_dir}" >/dev/null
        return 1
    fi
    status="$(echo "${show_json}" | json_field "data.get('status','unknown')")"
    phase="$(echo "${show_json}" | json_field "data.get('phase','unknown')")"
    until_ready="$(echo "${show_json}" | json_field "data.get('until_ready', False)")"
    state_path="$(echo "${show_json}" | json_field "data.get('state_dir','')")"
    info "Progress: status=${status}, phase=${phase}, until_ready=${until_ready}"
    info "State path: ${state_path}"
    replay_text="$(${OMK_CMD} goal replay latest --format text 2>/dev/null || true)"
    if [[ -n "${replay_text}" ]]; then
        echo "${replay_text}" | head -n 20
    fi

    header "Step 5: proof-backed result"
    local proof_json proof_status readiness known_gaps
    if ! proof_json="$(${OMK_CMD} goal proof latest --format json 2>&1)"; then
        echo "${proof_json}"
        fail "omk goal proof latest --format json failed"
        cd "${start_dir}" >/dev/null
        return 1
    fi
    proof_status="$(echo "${proof_json}" | json_field "data.get('status','unknown')")"
    readiness="$(echo "${proof_json}" | json_field "data.get('readiness','unknown')")"
    known_gaps="$(echo "${proof_json}" | json_field "len(data.get('known_gaps', []))")"
    info "Proof status: ${proof_status}"
    info "Readiness: ${readiness}"
    info "Known gaps: ${known_gaps}"

    if [[ "${proof_status}" == "failed_infra" || "${proof_status}" == "cancelled" ]]; then
        fail "Proof status is terminal failure"
        cd "${start_dir}" >/dev/null
        return 1
    fi
    pass "Proof-backed result recorded"
    cd "${start_dir}" >/dev/null
}

header "Step 1: Setup"
OMK_CMD="$(find_omk)"
info "Using omk: ${OMK_CMD}"

if ! ${OMK_CMD} --version >/dev/null 2>&1 && ! ${OMK_CMD} version >/dev/null 2>&1; then
    fail "omk binary does not run. Try 'cargo build --release' first."
    exit 1
fi
pass "omk binary OK"

check_kimi
if ${USE_MOCK}; then
    pass "Running in MOCK mode (no real Kimi needed)"
else
    pass "Using real Kimi CLI"
fi

if ! supports_goal_runtime; then
    fail "omk goal runtime is unavailable. Install a current omk build before running this demo."
    exit 1
fi

prepare_project
run_goal_demo || EXIT_CODE=1

header "Step 6: Cleanup"
if [[ "${NORTH_STAR_NO_CLEANUP:-}" == "1" ]]; then
    info "NORTH_STAR_NO_CLEANUP=1 - keeping temp project"
    info "Temp dir kept at: ${DEMOS_DIR}"
    DEMOS_DIR=""
else
    info "Removing temp project..."
    cleanup
fi

header "North Star Demo Summary"
if [[ ${EXIT_CODE} -eq 0 ]]; then
    echo -e "${GREEN}All steps completed successfully.${RESET}"
else
    echo -e "${YELLOW}Some steps had issues (see above).${RESET}"
fi

echo ""
echo "Commands demonstrated:"
echo "  1. omk setup"
echo "  2. omk goal run \"<goal>\" --until-ready"
echo "  3. omk goal replay latest"
echo "  4. omk goal proof latest --format json"
echo ""
echo "Environment:"
echo "  Mock mode:  ${USE_MOCK}"
echo "  omk binary: ${OMK_CMD}"
echo ""

exit ${EXIT_CODE}
