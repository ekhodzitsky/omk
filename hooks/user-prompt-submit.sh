#!/usr/bin/env bash
# OMK UserPromptSubmit hook for Kimi CLI
# Install: copy to ~/.kimi/hooks/user-prompt-submit.sh and make executable
# Or configure via Kimi CLI config

set -euo pipefail

# Kimi CLI passes event JSON via stdin
EVENT_JSON=$(cat)
PROMPT=$(echo "$EVENT_JSON" | python3 -c "import sys,json; print(json.load(sys.stdin).get('prompt',''))" 2>/dev/null || echo "")

# Check for skill triggers (simple keyword matching)
# In production, omk CLI would manage this via its own injector

if echo "$PROMPT" | grep -qiE "\bteam\b|\borchestrate\b"; then
    echo '{"continue":true,"message":"Team mode detected. Use /skill:team for best results."}'
    exit 0
fi

if echo "$PROMPT" | grep -qiE "\bautopilot\b|\bbuild me\b"; then
    echo '{"continue":true,"message":"Autopilot mode detected. Use /skill:autopilot for best results."}'
    exit 0
fi

# Default: allow prompt to proceed
echo '{"continue":true}'
