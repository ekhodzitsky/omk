use std::path::Path;

use crate::kimi_native::diagnostics::{DiagResult, Severity};

#[derive(Debug, Clone, serde::Deserialize)]
struct HookConfigWrapper {
    #[serde(default)]
    hooks: Vec<crate::kimi_native::hook_spec::HookConfig>,
}

pub(super) async fn check_hooks(
    hooks_dir: &Path,
    project_dir: &Path,
    results: &mut Vec<DiagResult>,
) {
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

    // Validate hooks declared in .kimi/config.toml
    let config_path = project_dir.join(".kimi").join("config.toml");
    if config_path.exists() {
        match tokio::fs::read_to_string(&config_path).await {
            Ok(content) => match toml::from_str::<HookConfigWrapper>(&content) {
                Ok(wrapper) => {
                    for hook in &wrapper.hooks {
                        let cmd_path = project_dir.join(&hook.command);
                        if cmd_path.exists() {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if let Ok(meta) = tokio::fs::metadata(&cmd_path).await {
                                    let mode = meta.permissions().mode();
                                    if mode & 0o111 != 0 {
                                        results.push(DiagResult {
                                            severity: Severity::Ok,
                                            message: format!(
                                                "Hook '{}' is executable",
                                                hook.command
                                            ),
                                            fix_hint: None,
                                        });
                                    } else {
                                        results.push(DiagResult {
                                            severity: Severity::Warning,
                                            message: format!(
                                                "Hook '{}' is not executable",
                                                hook.command
                                            ),
                                            fix_hint: Some(format!(
                                                "chmod +x {}",
                                                cmd_path.display()
                                            )),
                                        });
                                    }
                                }
                            }
                            #[cfg(not(unix))]
                            {
                                results.push(DiagResult {
                                    severity: Severity::Ok,
                                    message: format!("Hook '{}' exists", hook.command),
                                    fix_hint: None,
                                });
                            }
                        } else {
                            results.push(DiagResult {
                                severity: Severity::Warning,
                                message: format!(
                                    "Hook '{}' references missing script",
                                    hook.command
                                ),
                                fix_hint: Some(format!(
                                    "Create {} or run `omk kimi sync`",
                                    cmd_path.display()
                                )),
                            });
                        }
                    }
                }
                Err(e) => {
                    results.push(DiagResult {
                        severity: Severity::Warning,
                        message: format!(
                            "Hook config '{}' is invalid TOML: {}",
                            config_path.display(),
                            e
                        ),
                        fix_hint: Some(format!("Review and fix {}", config_path.display())),
                    });
                }
            },
            Err(e) => {
                results.push(DiagResult {
                    severity: Severity::Warning,
                    message: format!("Cannot read hook config '{}': {}", config_path.display(), e),
                    fix_hint: None,
                });
            }
        }
    }
}

pub(super) async fn check_hook_configs(dir: &Path, kimi_dir: &Path, results: &mut Vec<DiagResult>) {
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
}
