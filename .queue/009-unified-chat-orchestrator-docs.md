---
id: 009
title: UNIFIED_CHAT — orchestrator decisions and base SHA
status: pr_open
branch: ws/unified-chat-orchestrator-docs
worktree: /tmp/omk-orchestrator-docs
blocked_by: []
merge_after: []
size: small
batch: unified-chat-wave-1
pr: 113
notes: Resolves §16 open questions and freezes base SHA for W1-W6 worktrees. Awaiting review.
---

# Orchestrator artifacts for UNIFIED_CHAT Wave 1

Two new docs landed via PR #113:

- `docs/UNIFIED_CHAT_DECISIONS.md` — answers the 8 §16 open questions (D1 medium-cap=3, D2 soft+optional-hard cost cap, D3 first-prompt threshold 0.85, D4 wire pool 3+fresh, D5 telemetry 30/90d, D6 pre-flight Q downgrade, D7 ask-with-5s-default-yes resume, D8 unknown-slash one-time hint) plus 5 coordination notes (CO-1 goal-mod collision avoidance, CO-2 cargo discipline, CO-3 spec-file-not-on-master, CO-4 classifier prompt redirect, CO-5 anti-goals are hard rules).
- `docs/UNIFIED_CHAT_BASE.md` — freezes base SHA `8425033` for W1-W6 + lists in-flight audit workstreams to avoid collision.

These two files are referenced by all six W-tasks (010-015). They can stay `pr_open` while workers proceed — workers inline the relevant decisions in their prompts, not via filesystem lookup.

Merge unblocks nothing technically (W1-W6 don't depend on these files being on master), but landing them keeps the audit trail clean.
