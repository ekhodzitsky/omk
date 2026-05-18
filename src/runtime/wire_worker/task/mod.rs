mod context_guard;
mod process;
mod recorder;

/// Outcome of [`WireWorkerAdapter::process_task`].
///
/// `Completed` means the task ran to a natural conclusion (success, failure,
/// or wire error) and the result has already been written to the outbox.
///
/// The `Cancelled*` variants both mean cancellation fired before completion —
/// kimi has been killed and no result has been written. They differ in which
/// token fired:
///
/// - [`TaskOutcome::CancelledTimeout`]: the per-task budget elapsed; caller
///   should record a timeout in the outbox so the scheduler sees a failure.
/// - [`TaskOutcome::CancelledExternal`]: the outer worker shutdown token
///   fired; caller should not record anything because the worker itself is
///   tearing down.
///
/// The variant is determined inside the `select!` arm that fires, which
/// eliminates the TOCTOU race that an after-the-fact `is_cancelled()` check
/// has when both tokens fire in quick succession.
pub(super) enum TaskOutcome {
    Completed,
    CancelledTimeout,
    CancelledExternal,
}
