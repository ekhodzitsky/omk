#!/usr/bin/env bash
set -euo pipefail

KIMI_BIN="${KIMI_BIN:-kimi}"

if ! command -v "$KIMI_BIN" >/dev/null 2>&1; then
  printf 'kimi binary not found. Set KIMI_BIN=/path/to/kimi or install Kimi CLI.\n' >&2
  exit 127
fi

protocol_version="$("$KIMI_BIN" info 2>/dev/null | awk -F': ' '/wire protocol/ {print $2; exit}')"
# Fallback is only for local smoke bootstrapping when `kimi info` is unavailable.
protocol_version="${protocol_version:-1.9}"

request=$(printf '{"jsonrpc":"2.0","id":"1","method":"initialize","params":{"protocol_version":"%s","client":{"name":"omk-wire-smoke","version":"0.0.0"},"capabilities":{"supports_question":false,"supports_plan_mode":false}}}' "$protocol_version")

printf 'Sending Wire initialize request to %s --wire with protocol %s\n' "$KIMI_BIN" "$protocol_version" >&2
printf '%s\n' "$request" | "$KIMI_BIN" --wire
