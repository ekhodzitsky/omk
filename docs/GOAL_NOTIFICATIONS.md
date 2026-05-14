# Goal Notification Extension Point

`omk goal` does not push, publish, merge, provision services, or send network
notifications implicitly. The no-dependency notification surface is the durable
goal state directory:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/omk/goals/<goal-id>/
```

Watchers can observe these files:

- `events.jsonl` for append-only lifecycle, task, gate, replay, pause, cancel,
  and budget events.
- `goal.json` for the current goal status and phase.
- `proof.json` for readiness, known gaps, review/oracle/integration evidence,
  and proof status.
- `failure.json` for cancelled or human/external blocked outcomes.
- `budget-checkpoints.jsonl` for `needs_more_budget` and `budget-add`
  transitions.

## Contract

A notification hook should be a local process that tails or polls those files.
It should treat JSON fields as public API and ignore unknown fields. It should
not write human text into machine-readable OMK streams. If it writes user-facing
messages, write them to its own log, terminal, or notification backend.

Recommended trigger points:

- `goal.status == "ready"`: alert that local integration accepted a proof.
- `goal.status == "blocked_on_human"`: alert with
  `proof.human_decisions_required`.
- `goal.status == "needs_more_budget"`: alert with the latest budget
  checkpoint.
- `goal.status == "cancelled"`, `failed_infra`, or `blocked_on_external`:
  alert with `failure.reason` or `proof.known_gaps`.

## Minimal Poller Shape

The portable baseline is a shell loop owned by the operator:

```sh
goal_dir="${XDG_STATE_HOME:-$HOME/.local/state}/omk/goals/$GOAL_ID"
last_status=""

while sleep 5; do
  [ -f "$goal_dir/goal.json" ] || continue
  status=$(sed -n 's/.*"status": "\([^"]*\)".*/\1/p' "$goal_dir/goal.json" | head -1)
  [ -n "$status" ] || continue
  [ "$status" = "$last_status" ] && continue
  last_status="$status"
  printf '%s goal %s status=%s\n' "$(date -Iseconds)" "$GOAL_ID" "$status"
done
```

For richer integrations, use an external JSON parser and read `proof.json` or
`budget-checkpoints.jsonl` before sending a Slack, Discord, email, desktop, or
CI notification.
