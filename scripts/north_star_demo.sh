#!/usr/bin/env bash
#
# North Star Demo Script for oh-my-kimi
#
# Demonstrates the full North Star flow:
#   omk kimi sync
#   omk team run "fix all failing tests and produce a proof"
#   omk hud
#   omk proof show latest
#
# This script is idempotent and safe — it only touches a temporary directory.
# Set MOCK_KIMI=1 to use a wire-compatible mock instead of real Kimi CLI.
#

set -euo pipefail

# ---------------------------------------------------------------------------
# Colors
# ---------------------------------------------------------------------------
RESET='\033[0m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'

pass() { echo -e "${GREEN}✓${RESET} $*"; }
fail() { echo -e "${RED}✗${RESET} $*"; }
info() { echo -e "${BLUE}ℹ${RESET} $*"; }
warn() { echo -e "${YELLOW}⚠${RESET} $*"; }
header() { echo -e "\n${BOLD}$*${RESET}\n${BOLD}$(printf '=%.0s' $(seq 1 ${#1}))${RESET}"; }

# ---------------------------------------------------------------------------
# Tracking
# ---------------------------------------------------------------------------
EXIT_CODE=0
DEMOS_DIR=""
TEAM_NAME="north-star-demo"
USE_MOCK=false
MOCK_KIMI_PATH=""
OMK_CMD=""

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

cleanup() {
    if [[ -n "${DEMOS_DIR}" && -d "${DEMOS_DIR}" ]]; then
        if ${USE_MOCK}; then
            # With mock, team run state is written; clean it up quietly
            ${OMK_CMD} team cleanup --all >/dev/null 2>&1 || true
        fi
        rm -rf "${DEMOS_DIR}"
        pass "Cleaned up temp directory"
    fi
}

trap cleanup EXIT

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

    # Fall back to cargo run (slower but works from a fresh clone)
    echo "cargo run --quiet --release --bin omk --"
}

check_kimi() {
    if [[ -n "${MOCK_KIMI:-}" ]] || [[ "${USE_MOCK:-false}" == "true" ]]; then
        USE_MOCK=true
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

    # Check if the repo's mock-kimi binary exists
    local repo_root
    repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    if [[ -x "${repo_root}/target/debug/mock-kimi" ]] || [[ -x "${repo_root}/target/release/mock-kimi" ]]; then
        USE_MOCK=true
        return
    fi

    fail "Neither 'kimi' nor 'mock-kimi' found."
    info "  Install Kimi CLI: https://github.com/MoonshotAI/kimi-cli"
    info "  Or run with MOCK_KIMI=1 for a fully mocked demo"
    exit 1
}

# ---------------------------------------------------------------------------
# Wire-compatible mock Kimi (Python)
# ---------------------------------------------------------------------------
write_mock_kimi() {
    cat > "${DEMOS_DIR}/mock-kimi-wire" <<'PYEOF'
#!/usr/bin/env python3
import sys
import json
import argparse

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--wire', action='store_true')
    parser.add_argument('--work-dir', type=str)
    parser.add_argument('--version', action='store_true')
    args, _ = parser.parse_known_args()

    if args.version:
        print("kimi version 0.1.0-mock")
        return

    if not args.wire:
        # Fallback to original mock-kimi -p behaviour
        if len(sys.argv) >= 3 and sys.argv[1] == '-p':
            with open(sys.argv[2]) as f:
                prompt = f.read()
            print(json.dumps({
                "status": "success",
                "mock": True,
                "prompt_preview": prompt[:80],
                "response": "Mock Kimi response."
            }))
        else:
            print("Usage: mock-kimi --wire", file=sys.stderr)
            sys.exit(1)
        return

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue

        # Ignore responses to our own requests
        if 'id' in msg and ('result' in msg or 'error' in msg):
            continue

        if msg.get('method') == 'initialize':
            resp = {"jsonrpc": "2.0", "id": msg["id"], "result": {"protocol_version": "1.9"}}
            print(json.dumps(resp), flush=True)
        elif msg.get('method') == 'prompt':
            resp = {"jsonrpc": "2.0", "id": msg["id"], "result": {"status": "ok"}}
            print(json.dumps(resp), flush=True)

            user_input = msg.get('params', {}).get('user_input', {})
            text = user_input.get('Text', '') if isinstance(user_input, dict) else str(user_input)
            text_lower = text.lower()

            if 'plan' in text_lower or 'subtask' in text_lower or 'break down' in text_lower:
                response_text = (
                    '[{"id":"task-1","description":"Fix the add function in src/lib.rs"},'
                    '{"id":"task-2","description":"Run cargo test to verify the fix"}]'
                )
            elif 'synthesis' in text_lower or 'summarize' in text_lower:
                response_text = (
                    'The failing test was fixed by correcting the expected value in the '
                    'assertion, and all tests now pass.'
                )
            else:
                response_text = (
                    'Fixed the failing test by updating the assertion to expect 4 '
                    'instead of 5. All tests pass.'
                )

            print(json.dumps({
                "jsonrpc": "2.0",
                "method": "event",
                "params": {"type": "text", "payload": {"text": response_text}}
            }), flush=True)
            print(json.dumps({
                "jsonrpc": "2.0",
                "method": "event",
                "params": {"type": "turn_end", "payload": {}}
            }), flush=True)
            return

if __name__ == '__main__':
    main()
PYEOF
    chmod +x "${DEMOS_DIR}/mock-kimi-wire"
}

# ---------------------------------------------------------------------------
# 1. Setup
# ---------------------------------------------------------------------------
header "Step 1: Setup"

OMK_CMD="$(find_omk)"
info "Using omk: ${OMK_CMD}"

# Verify omk works
# Support both Clap-style `--version` and subcommand-style `version`.
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

# Create temp project
DEMOS_DIR="$(mktemp -d /tmp/omk-north-star-XXXXXX)"
info "Temp project: ${DEMOS_DIR}"

# In mock mode, isolate runtime state in the temp dir for reproducible local runs.
# This avoids permission problems in restricted environments.
if ${USE_MOCK}; then
    export HOME="${DEMOS_DIR}/home"
    export XDG_STATE_HOME="${DEMOS_DIR}/xdg_state"
    export XDG_CONFIG_HOME="${DEMOS_DIR}/xdg_config"
    export XDG_CACHE_HOME="${DEMOS_DIR}/xdg_cache"
    mkdir -p "${HOME}" "${XDG_STATE_HOME}" "${XDG_CONFIG_HOME}" "${XDG_CACHE_HOME}"
    info "Using isolated HOME/XDG paths for MOCK mode"
fi

mkdir -p "${DEMOS_DIR}/src"

cat > "${DEMOS_DIR}/Cargo.toml" <<EOF
[package]
name = "north-star-fixture"
version = "0.1.0"
edition = "2021"
EOF

cat > "${DEMOS_DIR}/src/lib.rs" <<'RUSTEOF'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        // Intentionally wrong assertion for the demo
        assert_eq!(add(2, 2), 5);
    }
}
RUSTEOF

pass "Created temp Rust project with intentional failing test"

# Write wire-compatible mock if needed
if ${USE_MOCK}; then
    write_mock_kimi
    MOCK_KIMI_PATH="${DEMOS_DIR}/mock-kimi-wire"
    export MOCK_KIMI="${MOCK_KIMI_PATH}"
    info "MOCK_KIMI=${MOCK_KIMI_PATH}"
fi

# Verify the test actually fails
cd "${DEMOS_DIR}"
set +e
cargo test >/dev/null 2>&1
FIXTURE_TEST_EXIT=$?
set -e
if [[ ${FIXTURE_TEST_EXIT} -ne 0 ]]; then
    pass "cargo test confirms the fixture fails (as expected)"
else
    warn "cargo test did not fail as expected — proceeding anyway"
fi
cd - >/dev/null

# ---------------------------------------------------------------------------
# 2. omk kimi sync
# ---------------------------------------------------------------------------
header "Step 2: omk kimi sync"

SYNC_DRY_RUN=""
if [[ "${NORTH_STAR_DRY_RUN:-}" == "1" ]]; then
    SYNC_DRY_RUN="--dry-run"
    info "NORTH_STAR_DRY_RUN=1 — using --dry-run"
fi

if ${OMK_CMD} kimi sync --dir "${DEMOS_DIR}" ${SYNC_DRY_RUN} >/dev/null 2>&1; then
    pass "omk kimi sync completed"
else
    warn "omk kimi sync had issues (non-fatal for demo)"
fi

# ---------------------------------------------------------------------------
# 3. omk team run
# ---------------------------------------------------------------------------
header "Step 3: omk team run"

info "Launching team '${TEAM_NAME}' with 2 coder workers..."
info "Task: fix the failing test and make cargo test pass"

cd "${DEMOS_DIR}"
if ${OMK_CMD} team run \
    --name "${TEAM_NAME}" \
    --dir "${DEMOS_DIR}" \
    2:coder \
    "fix the failing test and make cargo test pass" 2>&1; then
    pass "omk team run completed"
else
    fail "omk team run failed"
    EXIT_CODE=1
fi
cd - >/dev/null

# ---------------------------------------------------------------------------
# 4. omk hud
# ---------------------------------------------------------------------------
header "Step 4: omk hud"

HUD_OUTPUT=""
if HUD_OUTPUT="$(${OMK_CMD} hud "${TEAM_NAME}" --once --json 2>/dev/null)"; then
    pass "omk hud --once --json completed"
    # Pretty-print a summary from the JSON
    TOTAL=$(echo "${HUD_OUTPUT}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('task_summary',{}).get('total',0))" 2>/dev/null || echo "?")
    COMPLETED=$(echo "${HUD_OUTPUT}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('task_summary',{}).get('completed',0))" 2>/dev/null || echo "?")
    WORKERS=$(echo "${HUD_OUTPUT}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d.get('workers',[])))" 2>/dev/null || echo "?")
    info "HUD snapshot: ${COMPLETED}/${TOTAL} tasks done, ${WORKERS} workers"
else
    fail "omk hud failed"
    EXIT_CODE=1
fi

# ---------------------------------------------------------------------------
# 5. omk proof show latest
# ---------------------------------------------------------------------------
header "Step 5: omk proof show latest"

PROOF_JSON=""
if PROOF_JSON="$(${OMK_CMD} proof show latest --format json 2>/dev/null)"; then
    pass "omk proof show latest completed"

    PROOF_STATUS="$(echo "${PROOF_JSON}" | python3 -c "import json,sys; print(json.load(sys.stdin).get('status','unknown'))" 2>/dev/null || echo "unknown")"
    PROOF_CHANGED_FILES="$(echo "${PROOF_JSON}" | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('changed_files',[])))" 2>/dev/null || echo "0")"
    PROOF_GATES_TOTAL="$(echo "${PROOF_JSON}" | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('gates',[])))" 2>/dev/null || echo "0")"
    PROOF_FAILURES_TOTAL="$(echo "${PROOF_JSON}" | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('failures',[])))" 2>/dev/null || echo "0")"
    PROOF_RETRIES_TOTAL="$(echo "${PROOF_JSON}" | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('retries',[])))" 2>/dev/null || echo "0")"
    PROOF_KNOWN_GAPS_TOTAL="$(echo "${PROOF_JSON}" | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('known_gaps',[])))" 2>/dev/null || echo "0")"

    info "Proof status: ${PROOF_STATUS}"
    info "Proof counts: changed_files=${PROOF_CHANGED_FILES}, gates=${PROOF_GATES_TOTAL}, failures=${PROOF_FAILURES_TOTAL}, retries=${PROOF_RETRIES_TOTAL}, known_gaps=${PROOF_KNOWN_GAPS_TOTAL}"

    if [[ "${PROOF_STATUS}" == "ready" ]]; then
        pass "Proof status is Ready"
    elif [[ "${PROOF_STATUS}" == "not_ready" ]]; then
        warn "Proof status is NotReady (follow-up needed)"
    elif [[ "${PROOF_STATUS}" == "failed" ]]; then
        fail "Proof status is Failed"
        EXIT_CODE=1
    else
        warn "Proof status is unclear from output"
    fi

    # Show human-readable text snippet too.
    PROOF_TEXT_OUTPUT="$(${OMK_CMD} proof show latest --format text 2>/dev/null || true)"
    if [[ -n "${PROOF_TEXT_OUTPUT}" ]]; then
        echo "${PROOF_TEXT_OUTPUT}" | head -n 25
    fi
else
    fail "omk proof show latest failed"
    EXIT_CODE=1
fi

# ---------------------------------------------------------------------------
# 6. Cleanup prompt
# ---------------------------------------------------------------------------
header "Step 6: Cleanup"

if [[ "${NORTH_STAR_NO_CLEANUP:-}" == "1" ]]; then
    info "NORTH_STAR_NO_CLEANUP=1 — skipping cleanup prompt"
    info "  Team state kept at: ~/.local/state/omk/team/${TEAM_NAME}"
    info "  Temp dir kept at:   ${DEMOS_DIR}"
    # Disable trap so we don't delete on exit
    DEMOS_DIR=""
else
    info "Removing team state and temp directory..."
    ${OMK_CMD} team cleanup --all --dry-run >/dev/null 2>&1 || true
    ${OMK_CMD} team cleanup --all >/dev/null 2>&1 || true
    pass "Cleanup done"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
header "North Star Demo Summary"

if [[ ${EXIT_CODE} -eq 0 ]]; then
    echo -e "${GREEN}All steps completed successfully.${RESET}"
else
    echo -e "${YELLOW}Some steps had issues (see above).${RESET}"
fi

echo ""
echo "Commands demonstrated:"
echo "  1. omk kimi sync"
echo "  2. omk team run 2:coder \"fix the failing test...\""
echo "  3. omk hud ${TEAM_NAME} --once --json"
echo "  4. omk proof show latest"
echo ""
echo "Environment:"
echo "  Mock mode:  ${USE_MOCK}"
echo "  omk binary: ${OMK_CMD}"
echo ""

exit ${EXIT_CODE}
