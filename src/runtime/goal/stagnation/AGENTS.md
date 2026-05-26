# Stagnation Module — Agent Rules

## Hard Constraints

1. **No panics in production code.** `unwrap`/`expect`/`panic!` are banned outside `#[cfg(test)]`.
2. **Preserve error source chains.** `RecoveryCheckpointError` and `StagnationCollectorError` must use `#[source]` (not `String` wrapping) so operators can trace full cause chains.
3. **Deterministic diagnosis.** The same history must always produce the same diagnosis. No randomness, no thread-local state, no time-dependent heuristics.
4. **Bounded autonomy.** Recovery plans are proposed to the operator, never auto-executed. Auto-execution only with `--auto-recover` flag (not yet implemented).
5. **Non-blocking detection.** Detection logic is pure/synchronous. Async I/O (checkpoint save/load) happens only at CLI boundaries.

## Invariants

- `coverage_delta == None` means "no data available", not "stagnant". The detector must return `false` for this metric when all values are `None`.
- `history.len() < warmup_iterations + window_size` → `None` (insufficient data).
- Proof score "complete" is defined as `1.0 - score <= proof_score_epsilon`, not `f64::EPSILON`.
- Stagnant metrics are detected by **range** (`max - min`), not endpoint difference (`last - first`), to avoid false positives on oscillation.

## Performance Notes

- `StagnationCollector` uses `VecDeque` for O(1) eviction at capacity.
- `levenshtein()` input is capped to `MAX_STDERR_SIMILARITY_LEN` (500 chars) before comparison.
- `detect_external_dependency_broken` groups failures by gate name to avoid O(n²) over all gates.
