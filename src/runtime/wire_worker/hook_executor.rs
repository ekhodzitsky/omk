use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tracing::warn;

use crate::kimi_native::hook_spec::{default_project_hooks, HookConfig};
use crate::wire::protocol::{redact_wire_secrets, HookAction, HookRequest, WireHookSubscription};

/// Discover active hook subscriptions for the given project directory.
///
/// Reads `.kimi/config.toml` if it exists, otherwise falls back to
/// `default_project_hooks()`. Only includes subscriptions whose referenced
/// script file exists and is executable.
pub async fn discover_hook_subscriptions(project_dir: Option<&Path>) -> Vec<WireHookSubscription> {
    discover_active_hooks(project_dir)
        .await
        .into_iter()
        .map(|h| h.subscription)
        .collect()
}

/// Internal representation of a discovered hook with its resolved script path.
struct ActiveHook {
    subscription: WireHookSubscription,
    script_path: PathBuf,
    regex: Option<regex::Regex>,
}

/// TOML wrapper for parsing the `[[hooks]]` table.
#[derive(Debug, Clone, serde::Deserialize)]
struct HookConfigWrapper {
    #[serde(default)]
    hooks: Vec<HookConfig>,
}

async fn discover_active_hooks(project_dir: Option<&Path>) -> Vec<ActiveHook> {
    let project_dir = match project_dir {
        Some(p) => p,
        None => return Vec::new(),
    };

    let config_path = project_dir.join(".kimi").join("config.toml");
    let hooks = match tokio::fs::try_exists(&config_path).await {
        Ok(true) => match tokio::fs::read_to_string(&config_path).await {
            Ok(content) => match toml::from_str::<HookConfigWrapper>(&content) {
                Ok(wrapper) => wrapper.hooks,
                Err(e) => {
                    warn!(path = %config_path.display(), error = %e, "Malformed hook config; falling back to defaults");
                    Vec::new()
                }
            },
            Err(e) => {
                warn!(path = %config_path.display(), error = %e, "Cannot read hook config; falling back to defaults");
                Vec::new()
            }
        },
        _ => {
            let defs = default_project_hooks();
            defs.hooks
        }
    };

    let mut active = Vec::new();
    for hook in hooks {
        let script_path = project_dir.join(&hook.command);
        if !is_executable(&script_path).await {
            continue;
        }

        let event_str = serde_json::to_string(&hook.event)
            .ok()
            .and_then(|s| serde_json::from_str::<String>(&s).ok())
            .unwrap_or_default();

        let id = if let Some(ref matcher) = hook.matcher {
            format!("{}-{}", pascal_to_snake(&event_str), matcher.to_lowercase())
        } else {
            pascal_to_snake(&event_str)
        };

        let timeout = hook.timeout.map(|t| t.clamp(1, 300) as u32).unwrap_or(30);

        let regex = hook.matcher.as_ref().and_then(|pattern| {
            match regex::Regex::new(pattern) {
                Ok(re) => Some(re),
                Err(e) => {
                    warn!(pattern = %pattern, error = %e, "Invalid hook matcher regex; matcher will be ignored");
                    None
                }
            }
        });

        active.push(ActiveHook {
            subscription: WireHookSubscription {
                id,
                event: event_str,
                matcher: hook.matcher,
                timeout: Some(timeout),
            },
            script_path,
            regex,
        });
    }

    active
}

/// Convert a PascalCase string to snake_case.
fn pascal_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

/// Check whether a path exists and is executable (on Unix, checks permissions;
/// on non-Unix, checks existence only).
async fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match tokio::fs::metadata(path).await {
            Ok(meta) => meta.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        match tokio::fs::try_exists(path).await {
            Ok(exists) => exists,
            Err(_) => false,
        }
    }
}

/// Executor for Kimi Wire Protocol hook requests.
#[derive(Debug, Clone)]
pub struct HookExecutor {
    project_dir: PathBuf,
}

impl HookExecutor {
    /// Create a new hook executor for the given project directory.
    pub fn new(project_dir: impl Into<PathBuf>) -> Self {
        Self {
            project_dir: project_dir.into(),
        }
    }

    /// Run the hook matching the given request.
    ///
    /// Discovers active subscriptions, finds the one matching the request's
    /// event and target, spawns the corresponding script, and translates the
    /// exit code into a `HookResult`.
    pub async fn run(&self, request: &HookRequest) -> anyhow::Result<HookResult> {
        let active_hooks = discover_active_hooks(Some(&self.project_dir)).await;

        let matched = active_hooks.iter().find(|h| {
            // Prefer matching by subscription_id if the request carries one.
            if !request.subscription_id.is_empty() && h.subscription.id == request.subscription_id {
                return true;
            }
            h.subscription.event == request.event
                && h.regex
                    .as_ref()
                    .map_or(true, |re| re.is_match(&request.target))
        });

        let matched = match matched {
            Some(m) => m,
            None => return Ok(HookResult::default_allow()),
        };

        let timeout =
            std::time::Duration::from_secs(matched.subscription.timeout.unwrap_or(30) as u64);

        let redacted_input = redact_wire_secrets(&request.input_data);
        let input_json = serde_json::to_string(&redacted_input)
            .context("Failed to serialize hook input data")?;

        let mut child = match tokio::process::Command::new(&matched.script_path)
            .current_dir(&self.project_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                return Ok(HookResult {
                    action: HookAction::Block,
                    reason: format!("failed to spawn hook: {e}"),
                });
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(input_json.as_bytes()).await {
                return Ok(HookResult {
                    action: HookAction::Block,
                    reason: format!("failed to write hook input: {e}"),
                });
            }
            if let Err(e) = stdin.flush().await {
                return Ok(HookResult {
                    action: HookAction::Block,
                    reason: format!("failed to flush hook input: {e}"),
                });
            }
            // Dropping stdin closes the pipe and signals EOF to the child.
        }

        let output_result = tokio::time::timeout(timeout, child.wait_with_output()).await;

        match output_result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

                match output.status.code() {
                    Some(0) => Ok(HookResult {
                        action: HookAction::Allow,
                        reason: stdout,
                    }),
                    Some(1) => Ok(HookResult {
                        action: HookAction::Block,
                        reason: if stdout.is_empty() { stderr } else { stdout },
                    }),
                    Some(code) => {
                        let detail = if stderr.is_empty() { &stdout } else { &stderr };
                        Ok(HookResult {
                            action: HookAction::Block,
                            reason: format!("hook exited with code {code}: {detail}"),
                        })
                    }
                    None => {
                        let detail = if stderr.is_empty() { &stdout } else { &stderr };
                        Ok(HookResult {
                            action: HookAction::Block,
                            reason: format!("hook terminated without exit code: {detail}"),
                        })
                    }
                }
            }
            Ok(Err(e)) => Ok(HookResult {
                action: HookAction::Block,
                reason: format!("failed to collect hook output: {e}"),
            }),
            Err(_) => Ok(HookResult {
                action: HookAction::Block,
                reason: format!("hook timed out after {timeout:?}"),
            }),
        }
    }
}

/// Result of executing a hook script.
#[derive(Debug, Clone)]
pub struct HookResult {
    pub action: HookAction,
    pub reason: String,
}

impl HookResult {
    /// Default allow result when no matching hook is configured.
    pub fn default_allow() -> Self {
        Self {
            action: HookAction::Allow,
            reason: "No matching hook configured.".to_string(),
        }
    }

    /// Serialize this result into a JSON response value for the wire protocol.
    pub fn to_response_value(&self, request_id: &str) -> Value {
        let action_str = match self.action {
            HookAction::Allow => "allow",
            HookAction::Block => "block",
        };
        serde_json::json!({
            "request_id": request_id,
            "action": action_str,
            "reason": &self.reason,
        })
    }
}
