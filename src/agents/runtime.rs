use super::parser::{load_agents_file, AgentsManifest};
use anyhow::Result;
use std::path::Path;

/// Load project-level AGENTS.md from the current directory hierarchy.
/// Searches from `start_dir` up to the root.
pub async fn load_project_agents(start_dir: &Path) -> Result<Option<AgentsManifest>> {
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        if let Some(manifest) = load_agents_file(dir).await? {
            return Ok(Some(manifest));
        }
        current = dir.parent();
    }
    Ok(None)
}

/// Build an injected context string from AGENTS.md for prompt enrichment.
pub fn inject_agents_context(manifest: &AgentsManifest, task: &str, role: &str) -> String {
    let mut context = String::new();

    if let Some(name) = &manifest.name {
        context.push_str(&format!("## Project: {}\n", name));
    }
    if let Some(description) = &manifest.description {
        context.push_str(&format!("{}\n\n", description));
    }

    if !manifest.agents.is_empty() {
        context.push_str("## Available Agent Roles\n\n");
        for agent in &manifest.agents {
            let tier = agent.tier.as_deref().unwrap_or("general");
            context.push_str(&format!(
                "- **{}** ({}): {}\n",
                agent.role, tier, agent.description
            ));
        }
        context.push('\n');
    }

    if !manifest.body.is_empty() {
        context.push_str("## Project Context\n\n");
        context.push_str(&manifest.body);
        context.push_str("\n\n");
    }

    context.push_str("## Current Task\n\n");
    context.push_str(&format!("**Role**: {}\n", role));
    context.push_str(&format!("**Task**: {}\n\n", task));

    context
}

/// Default AGENTS.md template for `omk setup`.
pub fn default_agents_md() -> &'static str {
    r#"---
name: ""
description: ""
agents:
  - role: architect
    description: System architecture and high-level design
    tier: senior
  - role: frontend
    description: UI/UX implementation and component design
    tier: mid
  - role: backend
    description: API design, database modeling, business logic
    tier: mid
  - role: security
    description: Security audits, vulnerability assessment, secure coding
    tier: senior
  - role: devops
    description: Infrastructure, CI/CD, deployment automation
    tier: senior
  - role: data
    description: Data pipelines, analytics, ML integration
    tier: senior
  - role: qa
    description: Test design, coverage analysis, bug triage
    tier: mid
---
# Project Context

Add project-specific context here. This section is injected into every agent prompt.
"#
}
