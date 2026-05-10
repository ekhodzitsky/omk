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
            description: "Kimi-native architecture role for boundaries, APIs, and ADR-quality design decisions."
                .to_string(),
            system_prompt: include_str!("../../.kimi/agents/architect/system.md").to_string(),
            tools: vec!["read".to_string(), "write".to_string(), "ask".to_string()],
            default_skills: vec!["architect".to_string()],
            suggested_worker_count: 1,
        }
    }

    pub fn executor() -> Self {
        Self {
            id: "executor".to_string(),
            name: "Executor".to_string(),
            description: "Kimi-native implementation role for focused delivery with tests and anti-slop discipline."
                .to_string(),
            system_prompt: include_str!("../../.kimi/agents/executor/system.md").to_string(),
            tools: vec!["read".to_string(), "write".to_string(), "shell".to_string(), "test".to_string()],
            default_skills: vec!["backend".to_string(), "qa".to_string()],
            suggested_worker_count: 2,
        }
    }

    pub fn verifier() -> Self {
        Self {
            id: "verifier".to_string(),
            name: "Verifier".to_string(),
            description: "Kimi-native verification role for evidence-first gates, regression checks, and proof quality."
                .to_string(),
            system_prompt: include_str!("../../.kimi/agents/verifier/system.md").to_string(),
            tools: vec!["read".to_string(), "shell".to_string(), "test".to_string()],
            default_skills: vec!["qa".to_string()],
            suggested_worker_count: 1,
        }
    }

    pub fn reviewer() -> Self {
        Self {
            id: "reviewer".to_string(),
            name: "Reviewer".to_string(),
            description: "Kimi-native review role for correctness, risk discovery, and actionable feedback before merge."
                .to_string(),
            system_prompt: include_str!("../../.kimi/agents/reviewer/system.md").to_string(),
            tools: vec!["read".to_string(), "ask".to_string()],
            default_skills: vec!["security-review".to_string()],
            suggested_worker_count: 1,
        }
    }

    pub fn integrator() -> Self {
        Self {
            id: "integrator".to_string(),
            name: "Integrator".to_string(),
            description: "Kimi-native release role for safe integration, CI gates, and release readiness checks."
                .to_string(),
            system_prompt: include_str!("../../.kimi/agents/integrator/system.md").to_string(),
            tools: vec!["shell".to_string(), "read".to_string(), "write".to_string()],
            default_skills: vec!["devops".to_string()],
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

    #[test]
    fn test_every_role_prompt_includes_hierarchy_and_anti_slop_guards() {
        let roles = RolePack::all();
        for role in roles {
            assert!(
                role.system_prompt.contains("Instruction Hierarchy"),
                "{} prompt must define instruction hierarchy",
                role.id
            );
            assert!(
                role.system_prompt.contains("AGENTS.md"),
                "{} prompt must mention AGENTS.md hierarchy",
                role.id
            );
            assert!(
                role.system_prompt.contains("Anti-Slop"),
                "{} prompt must include anti-slop discipline",
                role.id
            );
            assert!(
                role.system_prompt.contains("Review Discipline"),
                "{} prompt must include review discipline",
                role.id
            );
        }
    }
}
