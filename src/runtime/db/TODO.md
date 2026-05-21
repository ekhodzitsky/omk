# src/runtime/db/ TODO

## Done

- [x] Integration into `runtime/goal/` state and task graph storage

## Known Gaps

- [ ] No migration runner beyond v1 (`PRAGMA user_version` is tracked but
  upgrade paths are not yet implemented)
- [ ] No bulk insert for events (`append_batch` would reduce round-trips)
- [ ] No single-artifact delete by `artifact_id`
- [ ] No optimistic locking for `goals.version`
- [ ] `update_status` only updates `status` + `phase`; no full `GoalRecord`
  update method exists yet
- [ ] Event pagination by `created_at` alone can duplicate events at second
  granularity; consumers may need `event_id`-based cursors in future

## Ideas (not committed)

- [ ] Add `Serialize`/`Deserialize` to record types for JSON event streams
- [ ] Add `get_by_id` for goals, events, artifacts for symmetry
- [ ] Consider query-builder for complex filters if needs grow
