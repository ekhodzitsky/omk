use anyhow::Result;
use std::path::Path;
use tracing::{info, warn};

use super::agent_spec::{default_role_agents, AgentBody, AgentSpec};
use super::hook_spec::default_project_hooks;
use super::manifest::{is_identical, maybe_backup, AssetManifest, EntryKind};

/// Reconcile OMK assets with `.kimi/` directory.
/// Skips files that are byte-identical or checksum-identical.
pub async fn sync_project_assets(
    project_dir: &Path,
    force: bool,
    dry_run: bool,
) -> Result<SyncReport> {
    let mut report = SyncReport::project();
    let mut manifest = AssetManifest::new(project_dir);
    manifest.add_dir(&std::path::PathBuf::from(".kimi/agents"));
    manifest.add_dir(&std::path::PathBuf::from(".kimi/hooks"));

    // Sync agents
    let agents_dir = project_dir.join(".kimi").join("agents");
    for agent in default_role_agents() {
        let agent_dir = agents_dir.join(&agent.id);
        let spec_path = agent_dir.join("agent.yaml");
        let prompt_path = agent_dir.join("system.md");

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
        let yaml = spec.to_yaml().unwrap_or_default();

        let mut agent_unchanged = true;

        // Spec file
        if force || !is_identical(&spec_path, &yaml).await {
            agent_unchanged = false;
            if dry_run {
                if spec_path.exists() {
                    report
                        .would_update
                        .push(format!("agent/{}/agent.yaml", agent.id));
                } else {
                    report
                        .would_create
                        .push(format!("agent/{}/agent.yaml", agent.id));
                }
            } else {
                let existed = spec_path.exists();
                tokio::fs::create_dir_all(&agent_dir).await?;
                if let Some(backup) = maybe_backup(&spec_path, &yaml).await {
                    let backup_path = std::path::PathBuf::from(&backup);
                    report.backups_created.push(backup);
                    manifest.add_backup(&spec_path, &backup_path);
                }
                crate::runtime::atomic::atomic_write(&spec_path, yaml.as_bytes()).await?;
                if existed {
                    report
                        .updated
                        .push(format!("agent/{}/agent.yaml", agent.id));
                } else {
                    report
                        .created
                        .push(format!("agent/{}/agent.yaml", agent.id));
                }
            }
        }

        // Prompt file
        if force || !is_identical(&prompt_path, &agent.system_prompt).await {
            agent_unchanged = false;
            if dry_run {
                if prompt_path.exists() {
                    report
                        .would_update
                        .push(format!("agent/{}/system.md", agent.id));
                } else {
                    report
                        .would_create
                        .push(format!("agent/{}/system.md", agent.id));
                }
            } else {
                let existed = prompt_path.exists();
                tokio::fs::create_dir_all(&agent_dir).await?;
                if let Some(backup) = maybe_backup(&prompt_path, &agent.system_prompt).await {
                    let backup_path = std::path::PathBuf::from(&backup);
                    report.backups_created.push(backup);
                    manifest.add_backup(&prompt_path, &backup_path);
                }
                crate::runtime::atomic::atomic_write(&prompt_path, agent.system_prompt.as_bytes())
                    .await?;
                if existed {
                    report.updated.push(format!("agent/{}/system.md", agent.id));
                } else {
                    report.created.push(format!("agent/{}/system.md", agent.id));
                }
            }
        }

        if agent_unchanged {
            report.unchanged.push(format!("agent/{}", agent.id));
        }

        if !dry_run {
            manifest
                .add_file(
                    &std::path::PathBuf::from(format!(".kimi/agents/{}/agent.yaml", agent.id)),
                    EntryKind::AgentSpec,
                )
                .await;
            manifest
                .add_file(
                    &std::path::PathBuf::from(format!(".kimi/agents/{}/system.md", agent.id)),
                    EntryKind::AgentPrompt,
                )
                .await;
            manifest.add_dir(&std::path::PathBuf::from(format!(
                ".kimi/agents/{}",
                agent.id
            )));
        }
    }

    // Sync hooks
    let hooks_dir = project_dir.join(".kimi").join("hooks");
    let hook_defs = default_project_hooks();
    for (filename, content) in &hook_defs.scripts {
        let path = hooks_dir.join(filename);

        if force || !is_identical(&path, content).await {
            if dry_run {
                if path.exists() {
                    report.would_update.push(format!("hooks/{}", filename));
                } else {
                    report.would_create.push(format!("hooks/{}", filename));
                }
            } else {
                let existed = path.exists();
                if let Some(backup) = maybe_backup(&path, content).await {
                    let backup_path = std::path::PathBuf::from(&backup);
                    report.backups_created.push(backup);
                    manifest.add_backup(&path, &backup_path);
                }
                tokio::fs::create_dir_all(&hooks_dir).await?;
                crate::runtime::atomic::atomic_write(&path, content.as_bytes()).await?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(0o755);
                    let _ = tokio::fs::set_permissions(&path, perms).await;
                }
                info!(script = %filename, "Synced hook script");
                if existed {
                    report.updated.push(format!("hooks/{}", filename));
                } else {
                    report.created.push(format!("hooks/{}", filename));
                }
            }
        } else {
            report.unchanged.push(format!("hooks/{}", filename));
        }

        if !dry_run {
            manifest
                .add_file(
                    &std::path::PathBuf::from(format!(".kimi/hooks/{}", filename)),
                    EntryKind::HookScript,
                )
                .await;
        }
    }

    // Save manifest
    if !dry_run {
        if let Err(e) = manifest.save(project_dir).await {
            warn!(error = %e, "Failed to save asset manifest");
        }
    }

    Ok(report)
}

/// Sync user-level assets (same as install, but with skip logic).
pub async fn sync_user_assets(force: bool, dry_run: bool) -> Result<SyncReport> {
    let config_dir = dirs::config_dir()
        .map(|d| d.join("kimi"))
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config/kimi"));

    let mut report = SyncReport::user();

    for agent in default_role_agents() {
        let agent_dir = config_dir.join("agents").join(&agent.id);
        let spec_path = agent_dir.join("agent.yaml");
        let prompt_path = agent_dir.join("system.md");

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
        let yaml = spec.to_yaml().unwrap_or_default();

        let mut agent_unchanged = true;

        if force || !is_identical(&spec_path, &yaml).await {
            agent_unchanged = false;
            if dry_run {
                if spec_path.exists() {
                    report
                        .would_update
                        .push(format!("user-agent/{}/agent.yaml", agent.id));
                } else {
                    report
                        .would_create
                        .push(format!("user-agent/{}/agent.yaml", agent.id));
                }
            } else {
                let existed = spec_path.exists();
                tokio::fs::create_dir_all(&agent_dir).await?;
                if let Some(backup) = maybe_backup(&spec_path, &yaml).await {
                    report.backups_created.push(backup);
                }
                crate::runtime::atomic::atomic_write(&spec_path, yaml.as_bytes()).await?;
                if existed {
                    report
                        .updated
                        .push(format!("user-agent/{}/agent.yaml", agent.id));
                } else {
                    report
                        .created
                        .push(format!("user-agent/{}/agent.yaml", agent.id));
                }
            }
        }

        if force || !is_identical(&prompt_path, &agent.system_prompt).await {
            agent_unchanged = false;
            if dry_run {
                if prompt_path.exists() {
                    report
                        .would_update
                        .push(format!("user-agent/{}/system.md", agent.id));
                } else {
                    report
                        .would_create
                        .push(format!("user-agent/{}/system.md", agent.id));
                }
            } else {
                let existed = prompt_path.exists();
                tokio::fs::create_dir_all(&agent_dir).await?;
                if let Some(backup) = maybe_backup(&prompt_path, &agent.system_prompt).await {
                    report.backups_created.push(backup);
                }
                crate::runtime::atomic::atomic_write(&prompt_path, agent.system_prompt.as_bytes())
                    .await?;
                if existed {
                    report
                        .updated
                        .push(format!("user-agent/{}/system.md", agent.id));
                } else {
                    report
                        .created
                        .push(format!("user-agent/{}/system.md", agent.id));
                }
            }
        }

        if agent_unchanged {
            report.unchanged.push(format!("user-agent/{}", agent.id));
        }
    }

    Ok(report)
}

#[derive(Debug, Clone, Default)]
pub struct SyncReport {
    pub scope: SyncScope,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: Vec<String>,
    pub errors: Vec<String>,
    pub backups_created: Vec<String>,
    pub would_create: Vec<String>,
    pub would_update: Vec<String>,
}

impl SyncReport {
    pub fn project() -> Self {
        Self {
            scope: SyncScope::Project,
            ..Self::default()
        }
    }

    pub fn user() -> Self {
        Self {
            scope: SyncScope::User,
            ..Self::default()
        }
    }

    pub fn files_written(&self) -> usize {
        self.created.len() + self.updated.len()
    }

    pub fn files_unchanged(&self) -> usize {
        self.unchanged.len()
    }

    pub fn files_planned(&self) -> usize {
        self.would_create.len() + self.would_update.len()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SyncScope {
    #[default]
    Project,
    User,
}

impl SyncScope {
    pub fn as_label(self) -> &'static str {
        match self {
            SyncScope::Project => "Project-level",
            SyncScope::User => "User-level",
        }
    }
}
