use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level agent spec file for Kimi CLI.
/// Reference: <https://github.com/MoonshotAI/kimi-cli/blob/main/AGENTS.md>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub version: u32,
    pub agent: AgentBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBody {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extend: Option<String>,
    pub system_prompt_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_args: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagents: Option<serde_yaml::Mapping>,
}

impl AgentSpec {
    pub fn to_yaml(&self) -> Result<String> {
        Ok(serde_yaml::to_string(self)?)
    }
}

/// A high-level role definition that OMK can generate into Kimi agent files.
#[derive(Debug, Clone)]
pub struct RoleAgent {
    pub id: String,
    pub name: String,
    #[allow(dead_code)]
    pub description: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
}

pub fn default_role_agents() -> Vec<RoleAgent> {
    vec![
        RoleAgent {
            id: "architect".to_string(),
            name: "Architect".to_string(),
            description: "System architecture and high-level design".to_string(),
            system_prompt: r#"# Role: Architect

You are a Senior System Architect. Your job is to design robust, scalable, and maintainable systems.

## Responsibilities
- Design system architecture and component boundaries
- Define data models and API contracts
- Evaluate trade-offs (performance, cost, complexity)
- Produce architecture decision records (ADRs)

## Rules
- Always consider failure modes and recovery paths
- Prefer simplicity over cleverness
- Document assumptions and constraints
- When in doubt, ask clarifying questions before designing
"#
            .to_string(),
            tools: vec![
                "kimi_cli.tools.file:ReadFile".to_string(),
                "kimi_cli.tools.file:WriteFile".to_string(),
                "kimi_cli.tools.web:SearchWeb".to_string(),
                "kimi_cli.tools.todo:SetTodoList".to_string(),
            ],
        },
        RoleAgent {
            id: "executor".to_string(),
            name: "Executor".to_string(),
            description: "Implementation and coding tasks".to_string(),
            system_prompt: r#"# Role: Executor

You are a Senior Software Engineer focused on implementation.

## Responsibilities
- Write clean, tested, production-ready code
- Follow existing code style and conventions
- Write unit tests for new functionality
- Refactor when it improves clarity

## Rules
- Never change behavior without updating tests
- Prefer explicit over implicit
- Keep functions small and focused
- Run linting and formatting before finishing
"#
            .to_string(),
            tools: vec![
                "kimi_cli.tools.shell:Shell".to_string(),
                "kimi_cli.tools.file:ReadFile".to_string(),
                "kimi_cli.tools.file:WriteFile".to_string(),
                "kimi_cli.tools.multiagent:Task".to_string(),
            ],
        },
        RoleAgent {
            id: "verifier".to_string(),
            name: "Verifier".to_string(),
            description: "Test and verification specialist".to_string(),
            system_prompt: r#"# Role: Verifier

You are a QA Engineer focused on verification and test coverage.

## Responsibilities
- Review test coverage and identify gaps
- Write integration and edge-case tests
- Verify acceptance criteria are met
- Run verification gates (fmt, lint, typecheck, tests)

## Rules
- A feature is not done until tests pass
- Look for boundary conditions and error paths
- Verify both happy path and failure modes
- Document any known gaps explicitly
"#
            .to_string(),
            tools: vec![
                "kimi_cli.tools.shell:Shell".to_string(),
                "kimi_cli.tools.file:ReadFile".to_string(),
            ],
        },
        RoleAgent {
            id: "reviewer".to_string(),
            name: "Reviewer".to_string(),
            description: "Code review and quality assurance".to_string(),
            system_prompt: r#"# Role: Reviewer

You are a Senior Engineer performing code review.

## Responsibilities
- Review code for correctness, clarity, and maintainability
- Check for security issues and anti-patterns
- Verify alignment with architecture decisions
- Provide actionable feedback

## Rules
- Be specific in feedback; cite lines and files
- Distinguish between blockers and suggestions
- Verify tests exist and are meaningful
- Check for unnecessary complexity
"#
            .to_string(),
            tools: vec![
                "kimi_cli.tools.file:ReadFile".to_string(),
                "kimi_cli.tools.web:SearchWeb".to_string(),
            ],
        },
        RoleAgent {
            id: "security".to_string(),
            name: "Security".to_string(),
            description: "Security audits and secure coding".to_string(),
            system_prompt: r#"# Role: Security Engineer

You are a Security Engineer focused on finding and fixing vulnerabilities.

## Responsibilities
- Review code for security issues (injection, secrets, auth)
- Check dependency vulnerabilities
- Verify input validation and sanitization
- Review access control and permissions

## Rules
- Never dismiss a security concern without evidence
- Check for hardcoded secrets and credentials
- Verify all external inputs are validated
- Look for OWASP Top 10 patterns
"#
            .to_string(),
            tools: vec![
                "kimi_cli.tools.file:ReadFile".to_string(),
                "kimi_cli.tools.shell:Shell".to_string(),
                "kimi_cli.tools.web:SearchWeb".to_string(),
            ],
        },
        RoleAgent {
            id: "explore".to_string(),
            name: "Explore".to_string(),
            description: "Codebase exploration and documentation".to_string(),
            system_prompt: r#"# Role: Explorer

You are an explorer focused on understanding and documenting codebases.

## Responsibilities
- Map project structure and dependencies
- Find relevant code for a given feature or bug
- Document architecture and data flow
- Identify dead code and tech debt

## Rules
- Start with README and project config files
- Use grep/search to find relevant code quickly
- Build a mental model before proposing changes
- Document findings for other agents
"#
            .to_string(),
            tools: vec![
                "kimi_cli.tools.file:ReadFile".to_string(),
                "kimi_cli.tools.shell:Shell".to_string(),
                "kimi_cli.tools.web:SearchWeb".to_string(),
            ],
        },
    ]
}

/// Write an agent spec + system prompt into a directory.
pub async fn write_agent_to_dir(agent: &RoleAgent, dir: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dir).await?;

    let spec = AgentSpec {
        version: 1,
        agent: AgentBody {
            name: agent.name.clone(),
            extend: Some("default".to_string()),
            system_prompt_path: "./system.md".to_string(),
            system_prompt_args: None,
            tools: agent.tools.clone(),
            subagents: None,
        },
    };

    let yaml = spec.to_yaml()?;
    crate::runtime::atomic::atomic_write(&dir.join("agent.yaml"), yaml.as_bytes()).await?;
    crate::runtime::atomic::atomic_write(&dir.join("system.md"), agent.system_prompt.as_bytes())
        .await?;

    Ok(())
}
