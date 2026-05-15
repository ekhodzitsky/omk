use std::path::Path;

use crate::kimi_native::diagnostics::{DiagResult, Severity};

pub(super) async fn check_agents(agents_dir: &Path, results: &mut Vec<DiagResult>) {
    let expected_agents = [
        "architect",
        "executor",
        "verifier",
        "reviewer",
        "security",
        "explore",
    ];
    let mut missing_agents = vec![];
    for agent in &expected_agents {
        let agent_dir = agents_dir.join(agent);
        let spec = agent_dir.join("agent.yaml");
        let prompt = agent_dir.join("system.md");
        if spec.exists() && prompt.exists() {
            // Structural validation (L1-032)
            match tokio::fs::read_to_string(&spec).await {
                Ok(content) => {
                    match serde_yaml::from_str::<crate::kimi_native::agent_spec::AgentSpec>(
                        &content,
                    ) {
                        Ok(spec) => {
                            let mut issues = vec![];
                            if spec.version == 0 {
                                issues.push("missing or zero version");
                            }
                            if spec.agent.name.is_empty() {
                                issues.push("missing agent.name");
                            }
                            if spec.agent.system_prompt_path.is_empty() {
                                issues.push("missing agent.system_prompt_path");
                            }
                            if issues.is_empty() {
                                results.push(DiagResult {
                                    severity: Severity::Ok,
                                    message: format!("Agent '{}' spec is valid", agent),
                                    fix_hint: None,
                                });
                            } else {
                                results.push(DiagResult {
                                    severity: Severity::Warning,
                                    message: format!(
                                        "Agent '{}' spec invalid: {}",
                                        agent,
                                        issues.join(", ")
                                    ),
                                    fix_hint: Some(format!(
                                        "Run `omk kimi sync` to regenerate {}",
                                        agent
                                    )),
                                });
                            }
                        }
                        Err(e) => {
                            results.push(DiagResult {
                                severity: Severity::Warning,
                                message: format!("Agent '{}' spec is invalid YAML: {}", agent, e),
                                fix_hint: Some(format!(
                                    "Run `omk kimi sync` to regenerate {}",
                                    agent
                                )),
                            });
                        }
                    }
                }
                Err(e) => {
                    results.push(DiagResult {
                        severity: Severity::Error,
                        message: format!("Cannot read agent '{}' spec: {}", agent, e),
                        fix_hint: None,
                    });
                }
            }
        } else {
            missing_agents.push(*agent);
        }
    }

    if !missing_agents.is_empty() {
        results.push(DiagResult {
            severity: Severity::Warning,
            message: format!("Missing agents: {}", missing_agents.join(", ")),
            fix_hint: Some("Run `omk kimi install` or `omk kimi sync`".to_string()),
        });
    }
}
