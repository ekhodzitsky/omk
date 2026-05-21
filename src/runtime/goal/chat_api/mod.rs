mod adapter;
pub mod commands;
pub mod events;
pub mod handle;
mod registry;
pub mod source;
pub mod wire_pool;

pub use adapter::to_child_event;

pub use events::{ChildGoalEvent, PlanNode, PlanNodeStatus};
pub use handle::{ChildGoalConfig, ChildGoalHandle, CreateChildRequest};

use tokio::sync::broadcast;

/// Create a child goal attached to a chat session.
///
/// 1. Scaffolds the goal via existing `create_goal`.
/// 2. Writes a `chat_link.json` sidecar in the goal state dir.
/// 3. Spawns an event tailer on `events.jsonl`.
/// 4. Spawns `execute_goal` in the background.
/// 5. Registers the goal in the in-memory registry.
pub async fn create_child(req: CreateChildRequest) -> anyhow::Result<ChildGoalHandle> {
    use crate::runtime::goal::{create_goal, execute_goal, CreateGoalOptions, GoalDeliveryPolicy};

    let options = CreateGoalOptions {
        until_ready: false,
        budget_time: None,
        budget_tokens: None,
        budget_usd: req.config.max_budget_usd.map(|f| f as f64),
        max_agents: None,
        delivery_policy: GoalDeliveryPolicy::Local,
        merge_policy: req.config.merge_policy,
        slice_execution: false,
        enforce_protection: req.config.enforce_protection,
    };

    let state = create_goal(&req.prompt, options, None).await?;
    let goal_id = state.goal_id.clone();
    let session_id = req.session_id.clone();

    // Sidecar linking this goal to its parent chat session
    let link = serde_json::json!({
        "parent_session_id": req.session_id,
        "parent_conv_id": req.parent_conv_id,
        "child_goal_id": goal_id,
    });
    let link_path = state.state_dir.join("chat_link.json");
    tokio::fs::write(&link_path, serde_json::to_vec_pretty(&link)?).await?;

    let (sender, _receiver) = broadcast::channel(256);

    let tail_shutdown = tokio_util::sync::CancellationToken::new();

    let tail_task = tokio::spawn({
        let sender = sender.clone();
        let state_dir = state.state_dir.clone();
        let shutdown = tail_shutdown.clone();
        async move {
            let _ = source::tail_goal_events_into(state_dir, sender, shutdown).await;
        }
    });

    let exec_task = tokio::spawn({
        let goal_id = goal_id.clone();
        async move {
            let project_dir = match std::env::current_dir() {
                Ok(d) => d,
                Err(_) => return,
            };
            let _ = execute_goal(&goal_id, &project_dir).await;
            // Allow tail to drain final events before shutting down
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            tail_shutdown.cancel();
        }
    });

    registry::register(
        goal_id.clone(),
        registry::GoalEntry {
            sender: sender.clone(),
            tail_task,
            exec_task,
        },
    );

    Ok(ChildGoalHandle {
        goal_id,
        session_id,
        created_at: chrono::Utc::now(),
    })
}

/// Subscribe to events for an active child goal.
pub fn subscribe(goal_id: &str) -> anyhow::Result<broadcast::Receiver<ChildGoalEvent>> {
    let entry = registry::get(goal_id)
        .ok_or_else(|| anyhow::anyhow!("goal not found in chat_api registry: {}", goal_id))?;
    Ok(entry.sender.subscribe())
}

/// Pause an active child goal.
pub async fn pause(goal_id: &str) -> anyhow::Result<()> {
    crate::runtime::goal::pause_goal(goal_id).await?;
    Ok(())
}

/// Resume a paused child goal.
pub async fn resume(goal_id: &str) -> anyhow::Result<()> {
    crate::runtime::goal::resume_goal(goal_id).await?;
    Ok(())
}

/// Cancel an active child goal and remove it from the registry.
pub async fn cancel(goal_id: &str) -> anyhow::Result<()> {
    crate::runtime::goal::cancel_goal(goal_id).await?;
    registry::deregister(goal_id);
    Ok(())
}

/// Inject a hint into a running child goal.
///
/// **Not yet implemented in the underlying goal runtime.**
pub fn inject_hint(_goal_id: &str, _text: &str) -> anyhow::Result<()> {
    anyhow::bail!("not yet implemented in goal-runtime")
}
