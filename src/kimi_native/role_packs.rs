use serde::{Deserialize, Serialize};

/// A curated role pack defines agent behavior, default tools, and skill bundles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePack {
    pub id: String,
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub default_skills: Vec<String>,
    pub suggested_worker_count: usize,
}

impl RolePack {
    /// All built-in role packs.
    pub fn all() -> Vec<Self> {
        vec![
            Self::architect(),
            Self::executor(),
            Self::verifier(),
            Self::reviewer(),
            Self::integrator(),
        ]
    }

    pub fn architect() -> Self {
        Self {
            id: "architect".to_string(),
            name: "Architect".to_string(),
            description:
                "Designs system structure, APIs, and data models. Writes ADRs and interface specs."
                    .to_string(),
            system_prompt: include_str!("../../.kimi/agents/architect/system.md").to_string(),
            tools: vec!["read".to_string(), "write".to_string(), "ask".to_string()],
            default_skills: vec!["design-review".to_string()],
            suggested_worker_count: 1,
        }
    }

    pub fn executor() -> Self {
        Self {
            id: "executor".to_string(),
            name: "Executor".to_string(),
            description: "Implements features, writes tests, and fixes bugs. Fast and pragmatic.".to_string(),
            system_prompt: "You are an expert software engineer. Your job is to write clean, tested, production-ready code. Follow existing patterns in the codebase. Write tests for every change.".to_string(),
            tools: vec!["read".to_string(), "write".to_string(), "shell".to_string(), "test".to_string()],
            default_skills: vec!["test-driven".to_string()],
            suggested_worker_count: 2,
        }
    }

    pub fn verifier() -> Self {
        Self {
            id: "verifier".to_string(),
            name: "Verifier".to_string(),
            description: "Runs gates, checks proofs, and validates completeness. The QA layer.".to_string(),
            system_prompt: "You are a meticulous QA engineer. Verify that all requirements are met, tests pass, and no regressions were introduced. Produce a clear pass/fail report with evidence.".to_string(),
            tools: vec!["read".to_string(), "shell".to_string(), "test".to_string()],
            default_skills: vec!["gate-runner".to_string()],
            suggested_worker_count: 1,
        }
    }

    pub fn reviewer() -> Self {
        Self {
            id: "reviewer".to_string(),
            name: "Reviewer".to_string(),
            description: "Reviews code, docs, and design decisions. Catches issues before merge.".to_string(),
            system_prompt: "You are a senior code reviewer. Review changes for correctness, security, performance, and maintainability. Be constructive but rigorous.".to_string(),
            tools: vec!["read".to_string(), "ask".to_string()],
            default_skills: vec!["security-review".to_string()],
            suggested_worker_count: 1,
        }
    }

    pub fn integrator() -> Self {
        Self {
            id: "integrator".to_string(),
            name: "Integrator".to_string(),
            description: "Merges branches, resolves conflicts, and prepares releases.".to_string(),
            system_prompt: "You are a DevOps engineer. Handle git operations, resolve merge conflicts, run CI gates, and prepare clean releases.".to_string(),
            tools: vec!["shell".to_string(), "read".to_string(), "write".to_string()],
            default_skills: vec!["ship-it".to_string()],
            suggested_worker_count: 1,
        }
    }

    pub fn find(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|r| r.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_pack_all_has_5_roles() {
        let roles = RolePack::all();
        assert_eq!(roles.len(), 5);
    }

    #[test]
    fn test_role_pack_find_architect() {
        let pack = RolePack::find("architect");
        assert!(pack.is_some());
        let pack = pack.unwrap();
        assert_eq!(pack.id, "architect");
        assert_eq!(pack.name, "Architect");
        assert_eq!(pack.suggested_worker_count, 1);
    }

    #[test]
    fn test_role_pack_find_unknown_returns_none() {
        let pack = RolePack::find("nonexistent");
        assert!(pack.is_none());
    }

    #[test]
    fn test_executor_has_test_tool() {
        let pack = RolePack::find("executor").unwrap();
        assert!(pack.tools.contains(&"test".to_string()));
    }
}
