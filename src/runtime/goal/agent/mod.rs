mod monitor;
mod path_policy;
mod spawn;
mod types;

pub use path_policy::check_task_path_policy;
pub use types::{GoalAgentDispatchPlan, GoalAgentTaskProposal, GoalAgentWaveKind};

pub(crate) use monitor::{goal_agent_task_policy_payload, validate_goal_agent_task_proposals};
pub(crate) use spawn::{
    goal_agent_dispatch_plan, goal_agent_slice_dispatch_plan, proposal_from_task,
};
pub(crate) use types::{propose_goal_agent_tasks, GoalAgentTaskPolicy};

#[cfg(test)]
mod tests;
