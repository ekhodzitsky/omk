use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::backend::{CommandBackend, CommandResponse};
use super::help::render_help_table;
use super::parser::{parse_command, Command};
use super::registry::find_spec;

/// Shared mutable state for the command subsystem.
///
/// Kept separate from W1's `SessionState` so that W5 can evolve
/// independently until the orchestrator merges them on coordination day.
#[derive(Debug)]
pub struct CommandSessionState {
    pub unknown_command_hinted: AtomicBool,
    pub has_active_large_goal: AtomicBool,
}

impl Default for CommandSessionState {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandSessionState {
    pub fn new() -> Self {
        Self {
            unknown_command_hinted: AtomicBool::new(false),
            has_active_large_goal: AtomicBool::new(false),
        }
    }
}

/// Result of routing user input.
#[derive(Debug, Clone)]
pub enum InputDecision {
    /// A command was parsed and executed; here is the response.
    CommandHandled(CommandResponse),
    /// Send the raw text as a user message.
    SendAsText(String),
    /// Emit a one-time hint, then send the text.
    EmitHintThenSendAsText(String, String),
}

/// Dispatches parsed slash commands to the appropriate backend method.
pub struct CommandDispatcher {
    backend: Arc<dyn CommandBackend>,
    session: Arc<CommandSessionState>,
}

impl std::fmt::Debug for CommandDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandDispatcher")
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

impl CommandDispatcher {
    pub fn new(backend: Arc<dyn CommandBackend>, session: Arc<CommandSessionState>) -> Self {
        Self { backend, session }
    }

    /// Dispatch a parsed command.
    ///
    /// Validates argument counts against the registry, then routes to the
    /// backend or produces a local effect (e.g. theme switch, quit).
    pub async fn dispatch(&self, cmd: Command) -> CommandResponse {
        let Some(spec) = find_spec(&cmd.name) else {
            return CommandResponse::Error(format!("unknown command: /{}", cmd.name));
        };

        let arg_count = cmd.args.len() as u8;
        if arg_count < spec.min_args {
            return CommandResponse::Error(format!(
                "/{} requires at least {} argument(s)",
                cmd.name, spec.min_args
            ));
        }
        if let Some(max) = spec.max_args {
            if arg_count > max {
                return CommandResponse::Error(format!(
                    "/{} takes at most {} argument(s)",
                    cmd.name, max
                ));
            }
        }

        match spec.name {
            "help" => CommandResponse::Markdown(render_help_table()),
            "quick" => self.backend.dispatch_quick(&cmd.raw_args).await,
            "escalate" => self.backend.dispatch_escalate(&cmd.raw_args).await,
            "classify" => self.backend.dispatch_classify(&cmd.raw_args).await,
            "explain" => self.backend.dispatch_explain().await,
            "show" => match cmd.args.first().map(|s| s.as_str()) {
                Some("plan") => self.backend.dispatch_show_plan().await,
                Some("proof") => self.backend.dispatch_show_proof().await,
                Some("goals") => self.backend.dispatch_show_goals().await,
                _ => CommandResponse::Error("usage: /show <plan|proof|goals>".to_string()),
            },
            "goal" => {
                if cmd.args.len() == 2 && cmd.args[0] == "show" {
                    self.backend.dispatch_goal_show(&cmd.args[1]).await
                } else {
                    CommandResponse::Error("usage: /goal show <id>".to_string())
                }
            }
            "inject" => self.backend.dispatch_inject(&cmd.raw_args).await,
            "pause" => self.backend.dispatch_pause().await,
            "resume" => {
                if cmd.args.is_empty() {
                    self.backend.dispatch_resume().await
                } else {
                    self.backend.dispatch_resume_session(&cmd.args[0]).await
                }
            }
            "cancel" => self.backend.dispatch_cancel().await,
            "approve" => self.backend.dispatch_approve().await,
            "reject" => {
                self.backend
                    .dispatch_reject(cmd.args.first().map(|s| s.as_str()))
                    .await
            }
            "diff" => self.backend.dispatch_diff().await,
            "cost" => self.backend.dispatch_cost().await,
            "new" => self.backend.dispatch_new_session().await,
            "sessions" => self.backend.dispatch_list_sessions().await,
            "theme" => match cmd.args.first().map(|s| s.as_str()) {
                Some("dark") => CommandResponse::EffectThemeDark,
                Some("light") => CommandResponse::EffectThemeLight,
                _ => CommandResponse::Error("usage: /theme <dark|light>".to_string()),
            },
            "quit" | "exit" => CommandResponse::EffectExit,
            _ => CommandResponse::Error(format!("unknown command: /{}", cmd.name)),
        }
    }

    /// Attempt to parse `input` as a command; fall back to text (with D8
    /// hinting behaviour for unknown slash inputs).
    pub async fn route_input(&self, input: &str) -> InputDecision {
        if !input.starts_with('/') {
            return InputDecision::SendAsText(input.to_string());
        }

        match parse_command(input) {
            Ok(cmd) => {
                if find_spec(&cmd.name).is_some() {
                    InputDecision::CommandHandled(self.dispatch(cmd).await)
                } else {
                    let already_hinted = self.session.unknown_command_hinted.load(Ordering::SeqCst);
                    if already_hinted {
                        InputDecision::SendAsText(input.to_string())
                    } else {
                        self.session
                            .unknown_command_hinted
                            .store(true, Ordering::SeqCst);
                        let hint = "that's not a command; sending as text. use /help for available commands.".to_string();
                        InputDecision::EmitHintThenSendAsText(hint, input.to_string())
                    }
                }
            }
            Err(_) => InputDecision::SendAsText(input.to_string()),
        }
    }
}
