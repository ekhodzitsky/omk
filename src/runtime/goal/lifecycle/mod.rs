mod cleanup;
mod monitor;
mod start;
mod stop;
mod verify;

pub use monitor::review_goal;
pub use start::{execute_goal, execute_goal_with_dispatcher};
pub use verify::{verify_goal, verify_goal_with_slices};

pub(crate) use cleanup::{
    aggregate_agent_evidence, append_proof_event, ensure_goal_can_continue, merge_gate_results,
    process_slice_delivery_and_review,
};
pub(crate) use monitor::run_post_mutation_cycle;
pub(crate) use stop::{build_and_persist_execution_proof, finalize_execution_state};
