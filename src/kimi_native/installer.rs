use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use super::agent_spec::{default_role_agents, write_agent_to_dir, AgentBody, AgentSpec};
use super::hook_spec::{default_project_hooks, ProjectHookDefs};
use super::manifest::{AssetManifest, EntryKind};

/// Install Kimi-native project assets under `.kimi/` in the given directory.
pub async fn install_project_assets(project_dir: &Path, dry_run: bool) -> Result<InstallReport> {
    let mut report = InstallReport::default();
    let mut manifest = AssetManifest::new(project_dir);

    // Install agents
    let agents_dir = project_dir.join(".kimi").join("agents");
    manifest.add_dir(&PathBuf::from(".kimi/agents"));
    for agent in default_role_agents() {
        let agent_dir = agents_dir.join(&agent.id);
        let spec_path = agent_dir.join("agent.yaml");
        let prompt_path = agent_dir.join("system.md");

        if dry_run {
            report.would_install.push(format!("agent/{}", agent.id));
        } else {
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
            if let Ok(yaml) = spec.to_yaml() {
                if let Some(backup) = super::manifest::maybe_backup(&spec_path, &yaml).await {
                    report.backups_created.push(backup);
                }
            }
            if let Some(backup) =
                super::manifest::maybe_backup(&prompt_path, &agent.system_prompt).await
            {
                report.backups_created.push(backup);
            }

            match write_agent_to_dir(&agent, &agent_dir).await {
                Ok(()) => {
                    info!(agent = %agent.id, dir = %agent_dir.display(), "Installed agent");
                    report.agents_installed.push(agent.id.clone());
                    manifest
                        .add_file(
                            &PathBuf::from(format!(".kimi/agents/{}/agent.yaml", agent.id)),
                            EntryKind::AgentSpec,
                        )
                        .await;
                    manifest
                        .add_file(
                            &PathBuf::from(format!(".kimi/agents/{}/system.md", agent.id)),
                            EntryKind::AgentPrompt,
                        )
                        .await;
                    manifest.add_dir(&PathBuf::from(format!(".kimi/agents/{}", agent.id)));
                }
                Err(e) => {
                    warn!(agent = %agent.id, error = %e, "Failed to install agent");
                    report.errors.push(format!("agent {}: {}", agent.id, e));
                }
            }
        }
    }

    // Install hooks
    let hooks_dir = project_dir.join(".kimi").join("hooks");
    manifest.add_dir(&PathBuf::from(".kimi/hooks"));
    let hook_defs = default_project_hooks();
    for (filename, content) in &hook_defs.scripts {
        let path = hooks_dir.join(filename);
        if dry_run {
            report.would_install.push(format!("hooks/{}", filename));
        } else {
            tokio::fs::create_dir_all(&hooks_dir).await?;
            if let Some(backup) = super::manifest::maybe_backup(&path, content).await {
                report.backups_created.push(backup);
            }
            crate::runtime::atomic::atomic_write(&path, content.as_bytes()).await?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                let _ = tokio::fs::set_permissions(&path, perms).await;
            }
            info!(script = %filename, path = %path.display(), "Installed hook script");
            report.hooks_installed.push(filename.clone());
            manifest
                .add_file(
                    &PathBuf::from(format!(".kimi/hooks/{}", filename)),
                    EntryKind::HookScript,
                )
                .await;
        }
    }

    // Write hooks.toml reference for user to copy into config.toml
    let hooks_toml = hooks_toml_reference(&hook_defs);
    let hooks_toml_path = project_dir.join(".kimi").join("hooks.toml.example");
    if dry_run {
        report.would_install.push("hooks.toml.example".to_string());
    } else {
        if let Some(backup) = super::manifest::maybe_backup(&hooks_toml_path, &hooks_toml).await {
            report.backups_created.push(backup);
        }
        crate::runtime::atomic::atomic_write(&hooks_toml_path, hooks_toml.as_bytes()).await?;
        info!(path = %hooks_toml_path.display(), "Wrote hooks.toml.example");
        manifest
            .add_file(
                &PathBuf::from(".kimi/hooks.toml.example"),
                EntryKind::HookConfig,
            )
            .await;
    }

    // Install skills symlink if OMK skills exist
    let omk_skills_dir = crate::runtime::config::data_dir().join("skills");
    let kimi_skills_dir = project_dir.join(".kimi").join("skills");
    if omk_skills_dir.exists() {
        if dry_run {
            report.would_install.push("skills".to_string());
        } else {
            tokio::fs::create_dir_all(&kimi_skills_dir).await?;
            info!(src = %omk_skills_dir.display(), dst = %kimi_skills_dir.display(), "Linked skills");
            report.skills_linked = true;
            manifest.add_dir(&PathBuf::from(".kimi/skills"));
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

/// Install user-level Kimi assets under `~/.config/kimi/`.
#[allow(dead_code)]
pub async fn install_user_assets(dry_run: bool) -> Result<InstallReport> {
    let mut report = InstallReport::default();

    let config_dir = dirs::config_dir()
        .map(|d| d.join("kimi"))
        .unwrap_or_else(|| PathBuf::from("~/.config/kimi"));

    let agents_dir = config_dir.join("agents");
    for agent in default_role_agents() {
        let agent_dir = agents_dir.join(&agent.id);
        let spec_path = agent_dir.join("agent.yaml");
        let prompt_path = agent_dir.join("system.md");

        if dry_run {
            report
                .would_install
                .push(format!("user-agent/{}", agent.id));
        } else {
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
            if let Ok(yaml) = spec.to_yaml() {
                if let Some(backup) = super::manifest::maybe_backup(&spec_path, &yaml).await {
                    report.backups_created.push(backup);
                }
            }
            if let Some(backup) =
                super::manifest::maybe_backup(&prompt_path, &agent.system_prompt).await
            {
                report.backups_created.push(backup);
            }

            match write_agent_to_dir(&agent, &agent_dir).await {
                Ok(()) => {
                    info!(agent = %agent.id, dir = %agent_dir.display(), "Installed user agent");
                    report.agents_installed.push(agent.id.clone());
                }
                Err(e) => {
                    warn!(agent = %agent.id, error = %e, "Failed to install user agent");
                    report
                        .errors
                        .push(format!("user agent {}: {}", agent.id, e));
                }
            }
        }
    }

    Ok(report)
}

fn hooks_toml_reference(defs: &ProjectHookDefs) -> String {
    let mut toml = String::from(
        "# Copy these hooks into your ~/.kimi/config.toml\n\
         # or project .kimi/config.toml\n\n",
    );
    for hook in &defs.hooks {
        toml.push_str("[[hooks]]\n");
        toml.push_str(&format!("event = \"{:?}\"\n", hook.event));
        toml.push_str(&format!("command = \"{}\"\n", hook.command));
        if let Some(ref matcher) = hook.matcher {
            toml.push_str(&format!("matcher = \"{}\"\n", matcher));
        }
        if let Some(timeout) = hook.timeout {
            toml.push_str(&format!("timeout = {}\n", timeout));
        }
        toml.push('\n');
    }
    toml
}

#[derive(Debug, Clone, Default)]
pub struct InstallReport {
    pub agents_installed: Vec<String>,
    pub hooks_installed: Vec<String>,
    pub skills_linked: bool,
    pub errors: Vec<String>,
    pub backups_created: Vec<String>,
    pub would_install: Vec<String>,
}
