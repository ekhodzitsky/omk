use std::path::PathBuf;

use crate::runtime::goal::{
    accept_goal, reject_goal, resolve_goal, resolve_goal_proof,
    state::{goals_dir, GOAL_PROOF_FILE, GOAL_TECHNICAL_PLAN_FILE},
};

#[derive(Debug)]
pub struct ChildGoalSummary {
    pub goal_id: String,
    pub session_id: String,
    pub parent_conv_id: String,
    pub status: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ChatLink {
    parent_session_id: String,
    parent_conv_id: String,
    child_goal_id: String,
}

pub async fn show_proof(goal_id: &str) -> anyhow::Result<PathBuf> {
    let goal = resolve_goal(goal_id).await?;
    // Ensure proof exists by loading it
    let _proof = resolve_goal_proof(goal_id).await?;
    Ok(goal.state_dir.join(GOAL_PROOF_FILE))
}

pub async fn show_goals(session_id: &str) -> anyhow::Result<Vec<ChildGoalSummary>> {
    let goals_dir = goals_dir();
    let mut summaries = Vec::new();

    let mut entries = tokio::fs::read_dir(&goals_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let link_path = path.join("chat_link.json");
        if !link_path.exists() {
            continue;
        }
        let content = match tokio::fs::read_to_string(&link_path).await {
            Ok(c) => c,
            Err(_) => continue,
        };
        let link: ChatLink = match serde_json::from_str(&content) {
            Ok(l) => l,
            Err(_) => continue,
        };
        if link.parent_session_id == session_id {
            summaries.push(ChildGoalSummary {
                goal_id: link.child_goal_id,
                session_id: link.parent_session_id,
                parent_conv_id: link.parent_conv_id,
                status: "unknown".to_string(),
            });
        }
    }

    Ok(summaries)
}

pub async fn show_plan(goal_id: &str) -> anyhow::Result<String> {
    let goal = resolve_goal(goal_id).await?;
    let plan_path = goal.state_dir.join(GOAL_TECHNICAL_PLAN_FILE);
    let content = tokio::fs::read_to_string(&plan_path).await?;
    Ok(content)
}

pub async fn approve_slice(goal_id: &str) -> anyhow::Result<()> {
    let project_dir = std::env::current_dir()?;
    accept_goal(goal_id, "approved via chat_api", &project_dir).await?;
    Ok(())
}

pub async fn reject_slice(goal_id: &str, reason: Option<&str>) -> anyhow::Result<()> {
    let project_dir = std::env::current_dir()?;
    let reason = reason.unwrap_or("rejected via chat_api");
    reject_goal(goal_id, reason, &project_dir).await?;
    Ok(())
}
