use async_trait::async_trait;

/// Response variants produced by the command backend.
#[derive(Debug, Clone)]
pub enum CommandResponse {
    /// Plain text for the conversation log.
    Text(String),
    /// Markdown blob.
    Markdown(String),
    /// Successful execution with no visible output.
    Ok,
    /// Not yet wired — show the user what is pending.
    NotWired { reason: &'static str },
    /// Execution error.
    Error(String),
    /// Request to exit the application.
    EffectExit,
    /// Request to clear the conversation view.
    EffectClearView,
    /// Request to switch to dark theme.
    EffectThemeDark,
    /// Request to switch to light theme.
    EffectThemeLight,
    /// Request to start inline injection UI.
    EffectStartInjectInline,
    /// Request to start a new session.
    EffectStartNewSession,
}

/// Backend trait for executing slash commands.
///
/// Production implementations are provided by downstream workstreams
/// (W2 classifier, W3 router, W6 chat_api) and glued by the orchestrator
/// on coordination day.
#[async_trait]
pub trait CommandBackend: Send + Sync {
    // --- Override (router / W3) ---
    async fn dispatch_quick(&self, prompt: &str) -> CommandResponse;
    async fn dispatch_escalate(&self, prompt: &str) -> CommandResponse;

    // --- Classifier inspection (W2) ---
    async fn dispatch_classify(&self, prompt: &str) -> CommandResponse;
    async fn dispatch_explain(&self) -> CommandResponse;

    // --- Goal inspection (W6 chat_api) ---
    async fn dispatch_show_plan(&self) -> CommandResponse;
    async fn dispatch_show_proof(&self) -> CommandResponse;
    async fn dispatch_show_goals(&self) -> CommandResponse;
    async fn dispatch_goal_show(&self, goal_id: &str) -> CommandResponse;

    // --- Goal control (W6 chat_api) ---
    async fn dispatch_inject(&self, text: &str) -> CommandResponse;
    async fn dispatch_pause(&self) -> CommandResponse;
    async fn dispatch_resume(&self) -> CommandResponse;
    async fn dispatch_cancel(&self) -> CommandResponse;
    async fn dispatch_approve(&self) -> CommandResponse;
    async fn dispatch_reject(&self, reason: Option<&str>) -> CommandResponse;

    // --- Inspection ---
    async fn dispatch_diff(&self) -> CommandResponse;
    async fn dispatch_cost(&self) -> CommandResponse;

    // --- Session ---
    async fn dispatch_new_session(&self) -> CommandResponse;
    async fn dispatch_list_sessions(&self) -> CommandResponse;
    async fn dispatch_resume_session(&self, session_id: &str) -> CommandResponse;
}

/// Stub backend used in tests and until real backends are wired.
#[derive(Debug)]
pub struct StubBackend;

#[async_trait]
impl CommandBackend for StubBackend {
    async fn dispatch_quick(&self, _prompt: &str) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W3 router; not yet merged",
        }
    }

    async fn dispatch_escalate(&self, _prompt: &str) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W3 router; not yet merged",
        }
    }

    async fn dispatch_classify(&self, _prompt: &str) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W2 classifier integration; pending coordination",
        }
    }

    async fn dispatch_explain(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W2 classifier integration; pending coordination",
        }
    }

    async fn dispatch_show_plan(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W6 chat_api integration; pending coordination",
        }
    }

    async fn dispatch_show_proof(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W6 chat_api integration; pending coordination",
        }
    }

    async fn dispatch_show_goals(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W6 chat_api integration; pending coordination",
        }
    }

    async fn dispatch_goal_show(&self, _goal_id: &str) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W6 chat_api integration; pending coordination",
        }
    }

    async fn dispatch_inject(&self, _text: &str) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W6 chat_api integration; pending coordination",
        }
    }

    async fn dispatch_pause(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W6 chat_api integration; pending coordination",
        }
    }

    async fn dispatch_resume(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W6 chat_api integration; pending coordination",
        }
    }

    async fn dispatch_cancel(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W6 chat_api integration; pending coordination",
        }
    }

    async fn dispatch_approve(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W6 chat_api integration; pending coordination",
        }
    }

    async fn dispatch_reject(&self, _reason: Option<&str>) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W6 chat_api integration; pending coordination",
        }
    }

    async fn dispatch_diff(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires git diff helper; pending orchestrator wiring",
        }
    }

    async fn dispatch_cost(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W3 cost tracker; pending coordination",
        }
    }

    async fn dispatch_new_session(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W1 session manager; pending coordination",
        }
    }

    async fn dispatch_list_sessions(&self) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W1 session manager; pending coordination",
        }
    }

    async fn dispatch_resume_session(&self, _session_id: &str) -> CommandResponse {
        CommandResponse::NotWired {
            reason: "requires W1 session manager; pending coordination",
        }
    }
}
