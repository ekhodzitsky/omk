use anyhow::Result;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagResult {
    pub severity: Severity,
    pub message: String,
    pub fix_hint: Option<String>,
}

pub async fn diagnose_project(dir: &Path) -> Result<Vec<DiagResult>> {
    let mut results = Vec::new();
    let kimi_dir = dir.join(".kimi");
    let agents_dir = kimi_dir.join("agents");
    let hooks_dir = kimi_dir.join("hooks");

    // Check .kimi/ directory exists
    if kimi_dir.exists() {
        results.push(DiagResult {
            severity: Severity::Ok,
            message: format!(".kimi/ directory found at {}", kimi_dir.display()),
            fix_hint: None,
        });
    } else {
        results.push(DiagResult {
            severity: Severity::Warning,
            message: format!(".kimi/ directory not found at {}", kimi_dir.display()),
            fix_hint: Some("Run `omk kimi install` to create it".to_string()),
        });
    }

    // Check agents
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

    // Check hooks (L1-033)
    let expected_hooks = ["safety-check.sh", "completion-check.sh", "notify.sh"];
    let mut missing_hooks = vec![];
    for hook in &expected_hooks {
        let path = hooks_dir.join(hook);
        if path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = tokio::fs::metadata(&path).await {
                    let mode = meta.permissions().mode();
                    if mode & 0o111 != 0 {
                        results.push(DiagResult {
                            severity: Severity::Ok,
                            message: format!("Hook '{}' is executable", hook),
                            fix_hint: None,
                        });
                    } else {
                        results.push(DiagResult {
                            severity: Severity::Warning,
                            message: format!("Hook '{}' is not executable", hook),
                            fix_hint: Some(format!("chmod +x {}", path.display())),
                        });
                    }
                }
            }
            #[cfg(not(unix))]
            {
                results.push(DiagResult {
                    severity: Severity::Ok,
                    message: format!("Hook '{}' exists", hook),
                    fix_hint: None,
                });
            }
        } else {
            missing_hooks.push(*hook);
        }
    }

    if !missing_hooks.is_empty() {
        results.push(DiagResult {
            severity: Severity::Warning,
            message: format!("Missing hooks: {}", missing_hooks.join(", ")),
            fix_hint: Some("Run `omk kimi install` or `omk kimi sync`".to_string()),
        });
    }

    // Check hook configs reference existing scripts (L1-033)
    let hook_configs_to_check = ["hooks.toml.example", "config.toml"];
    for config_name in &hook_configs_to_check {
        let config_path = kimi_dir.join(config_name);
        if config_path.exists() {
            match tokio::fs::read_to_string(&config_path).await {
                Ok(content) => match toml::from_str::<HookConfigWrapper>(&content) {
                    Ok(wrapper) => {
                        let mut dangling = vec![];
                        for hook in &wrapper.hooks {
                            let cmd_path = dir.join(&hook.command);
                            if !cmd_path.exists() {
                                dangling.push(hook.command.clone());
                            }
                        }
                        if dangling.is_empty() {
                            results.push(DiagResult {
                                severity: Severity::Ok,
                                message: format!(
                                    "Hook config '{}' references valid scripts",
                                    config_name
                                ),
                                fix_hint: None,
                            });
                        } else {
                            results.push(DiagResult {
                                severity: Severity::Warning,
                                message: format!(
                                    "Hook config '{}' references missing scripts: {}",
                                    config_name,
                                    dangling.join(", ")
                                ),
                                fix_hint: Some(
                                    "Run `omk kimi sync` to restore missing hook scripts"
                                        .to_string(),
                                ),
                            });
                        }
                    }
                    Err(e) => {
                        results.push(DiagResult {
                            severity: Severity::Warning,
                            message: format!(
                                "Hook config '{}' is invalid TOML: {}",
                                config_name, e
                            ),
                            fix_hint: Some(format!("Review and fix {}", config_path.display())),
                        });
                    }
                },
                Err(e) => {
                    results.push(DiagResult {
                        severity: Severity::Warning,
                        message: format!("Cannot read hook config '{}': {}", config_name, e),
                        fix_hint: None,
                    });
                }
            }
        }
    }

    // Check skills paths (L1-034)
    let skills_dir = kimi_dir.join("skills");
    if skills_dir.exists() {
        if skills_dir.is_dir() {
            let mut invalid = vec![];
            let mut validated = 0usize;
            let mut entries = tokio::fs::read_dir(&skills_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let entry_path = entry.path();
                let entry_name = entry.file_name().to_string_lossy().to_string();
                let file_type = entry.file_type().await?;

                if !file_type.is_dir() {
                    invalid.push(format!("{} is not a directory", entry_path.display()));
                    continue;
                }

                let skill_md = entry_path.join("SKILL.md");
                if !skill_md.exists() {
                    invalid.push(format!("{}/SKILL.md is missing", entry_name));
                    continue;
                }

                validated += 1;
            }

            if invalid.is_empty() {
                results.push(DiagResult {
                    severity: Severity::Ok,
                    message: format!("Skills paths valid ({} skill(s))", validated),
                    fix_hint: None,
                });
            } else {
                results.push(DiagResult {
                    severity: Severity::Warning,
                    message: format!("Invalid skills paths: {}", invalid.join(", ")),
                    fix_hint: Some(
                        "Ensure each .kimi/skills/<name>/SKILL.md exists, or run `omk kimi sync`"
                            .to_string(),
                    ),
                });
            }
        } else {
            results.push(DiagResult {
                severity: Severity::Warning,
                message: format!(
                    "Invalid skills path: {} is not a directory",
                    skills_dir.display()
                ),
                fix_hint: Some("Remove it and run `omk kimi sync`".to_string()),
            });
        }
    }

    // Check for AGENTS.md
    let agents_md = dir.join("AGENTS.md");
    if agents_md.exists() {
        results.push(DiagResult {
            severity: Severity::Ok,
            message: "AGENTS.md found".to_string(),
            fix_hint: None,
        });
    } else {
        results.push(DiagResult {
            severity: Severity::Warning,
            message: "AGENTS.md not found".to_string(),
            fix_hint: Some("Create AGENTS.md with project conventions".to_string()),
        });
    }

    // Check for asset manifest
    let manifest_path = dir.join(".kimi").join("omk-manifest.json");
    if manifest_path.exists() {
        match crate::kimi_native::manifest::AssetManifest::load(dir).await {
            Ok(Some(manifest)) => {
                results.push(DiagResult {
                    severity: Severity::Ok,
                    message: format!(
                        "Asset manifest found (OMK v{}, {} files)",
                        manifest.omk_version,
                        manifest.files.len()
                    ),
                    fix_hint: None,
                });
                let drifted = manifest.drifted_files(dir).await;
                for (path, expected) in drifted {
                    match expected {
                        None => {
                            results.push(DiagResult {
                                severity: Severity::Warning,
                                message: format!("Missing manifest file: {}", path.display()),
                                fix_hint: Some(
                                    "Run `omk kimi sync` to restore missing files".to_string(),
                                ),
                            });
                        }
                        Some(checksum) => {
                            results.push(DiagResult {
                                severity: Severity::Warning,
                                message: format!(
                                    "File content drift detected: {} (expected: {})",
                                    path.display(),
                                    checksum
                                ),
                                fix_hint: Some(
                                    "Run `omk kimi sync` to restore drifted files".to_string(),
                                ),
                            });
                        }
                    }
                }

                if !manifest.backups.is_empty() {
                    results.push(DiagResult {
                        severity: Severity::Ok,
                        message: format!("Backup index entries: {}", manifest.backups.len()),
                        fix_hint: None,
                    });
                }

                for backup in &manifest.backups {
                    let backup_abs = dir.join(&backup.backup_path);
                    if !backup_abs.exists() {
                        results.push(DiagResult {
                            severity: Severity::Warning,
                            message: format!(
                                "Backup index drift: managed '{}' points to missing backup '{}'",
                                backup.managed_path.display(),
                                backup.backup_path.display()
                            ),
                            fix_hint: Some(
                                "Run `omk kimi sync --force` to refresh managed assets and backup index"
                                    .to_string(),
                            ),
                        });
                    }
                }
            }
            Ok(None) => {
                results.push(DiagResult {
                    severity: Severity::Warning,
                    message: "Asset manifest not found".to_string(),
                    fix_hint: Some("Run `omk kimi sync` to generate a manifest".to_string()),
                });
            }
            Err(e) => {
                results.push(DiagResult {
                    severity: Severity::Warning,
                    message: format!("Cannot read asset manifest: {}", e),
                    fix_hint: None,
                });
            }
        }
    } else {
        results.push(DiagResult {
            severity: Severity::Warning,
            message: "Asset manifest not found".to_string(),
            fix_hint: Some("Run `omk kimi sync` to generate a manifest".to_string()),
        });
    }

    // Check for Kimi CLI (L1-031)
    match which::which("kimi") {
        Ok(path) => {
            match tokio::process::Command::new("kimi")
                .arg("--version")
                .output()
                .await
            {
                Ok(output) if output.status.success() => {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    results.push(DiagResult {
                        severity: Severity::Ok,
                        message: format!("Kimi CLI {} at {}", version, path.display()),
                        fix_hint: None,
                    });
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let details = if stderr.is_empty() {
                        format!("exit status {}", output.status)
                    } else {
                        stderr
                    };
                    let repair = format!(
                        "Run `{0} --version`; if it still fails, reinstall Kimi CLI from https://www.kimi.com/code/docs and re-check with `command -v kimi && kimi --version`",
                        path.display()
                    );
                    results.push(DiagResult {
                        severity: Severity::Warning,
                        message: format!(
                            "Kimi CLI found at {} but version check failed: {}",
                            path.display(),
                            details
                        ),
                        fix_hint: Some(repair),
                    });
                }
                Err(e) => {
                    let repair = format!(
                        "Run `{0} --version`; if it still fails, reinstall Kimi CLI from https://www.kimi.com/code/docs and re-check with `command -v kimi && kimi --version`",
                        path.display()
                    );
                    results.push(DiagResult {
                        severity: Severity::Warning,
                        message: format!(
                            "Kimi CLI found at {} but version check could not run: {}",
                            path.display(),
                            e
                        ),
                        fix_hint: Some(repair),
                    });
                }
            }
        }
        Err(_) => {
            results.push(DiagResult {
                severity: Severity::Error,
                message: "Kimi CLI not found in PATH".to_string(),
                fix_hint: Some(
                    "Install Kimi CLI using https://www.kimi.com/code/docs, then run `command -v kimi && kimi --version`".to_string(),
                ),
            });
        }
    }

    Ok(results)
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HookConfigWrapper {
    #[serde(default)]
    hooks: Vec<crate::kimi_native::hook_spec::HookConfig>,
}
