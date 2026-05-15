# TODO — runtime::goal

## Done (Pilot 3)
- [x] Extract query functions (`list_goals`, `resolve_goal`, `resolve_goal_proof`) to `queries.rs`
- [x] Remove proxy re-exports from `dispatch/mod.rs` and `dispatch/tasks/mod.rs`
- [x] Flatten `mod.rs` to storefront (~80 lines)
- [x] Extract `GoalStateStore` trait + `FileSystemGoalStateStore` / `InMemoryGoalStateStore`
- [x] Remove `GoalState::save`/`load` methods; migrate all callers to store
- [x] Add unit tests for `queries.rs` (`InMemoryGoalStateStore` mock)
- [x] Split `planner.rs` into `planner/mod.rs` + `planner/scaffold.rs`
- [x] Extract `GoalDispatcher` trait; decouple `lifecycle.rs` from `dispatch/`
- [x] Add golden tests for proof reconciliation
- [x] Split `wave.rs` into `wave/{mod,policy,runner,results}.rs`

## Later
