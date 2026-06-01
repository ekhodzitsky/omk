use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::cli::chat::commands::backend::{CommandBackend, CommandResponse};
use crate::runtime::classifier::{ClassifierInput, Intent};
use crate::runtime::conversation::bus::EventBus;
use crate::runtime::conversation::outcome::RouteOutcome;
use crate::runtime::conversation::session::SessionCtx;
use crate::runtime::escalation::backends::{
    ClassifierBackend, GoalBridgeBackend, LlmDirectBackend, MediumPlanResult,
    ProductionClassifierBackend, ProductionGoalBridgeBackend, ProductionLlmDirectBackend,
    SmallEditResult, WireWorkerBackend,
};
use crate::runtime::escalation::router::{Router, RouterConfig};

/// Placeholder wire-worker backend until a real production implementation lands.
/// Returns explicit typed errors rather than silently no-opping.
#[derive(Debug)]
struct PlaceholderWireWorkerBackend;

#[async_trait]
impl WireWorkerBackend for PlaceholderWireWorkerBackend {
    async fn run_small_edit(&self, _task: &str) -> anyhow::Result<SmallEditResult> {
        anyhow::bail!("wire worker backend requires wire pool integration; pending next workstream")
    }

    async fn run_medium_plan(&self, _plan: &[String]) -> anyhow::Result<MediumPlanResult> {
        anyhow::bail!("wire worker backend requires wire pool integration; pending next workstream")
    }
}

/// Production backend composing W2 classifier, W3 router, and W6 chat_api.
pub struct ProductionBackend {
    router: Arc<Router>,
    classifier: Arc<dyn ClassifierBackend>,
    session: Arc<SessionCtx>,
    event_bus: Arc<EventBus>,
    latest_goal_id: Mutex<Option<String>>,
}

impl std::fmt::Debug for ProductionBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductionBackend")
            .field("router", &self.router)
            .field("session", &self.session)
            .field("event_bus", &self.event_bus)
            .finish_non_exhaustive()
    }
}

impl ProductionBackend {
    pub async fn build(session_id: String, project_root: PathBuf) -> anyhow::Result<Self> {
        let event_bus = Arc::new(EventBus::new());
        let session = SessionCtx::new(session_id.clone(), project_root.clone());

        let wire_client = crate::wire::ProcessWireClient::new(
            crate::wire::ChildProcessTransport::spawn("kimi", None, None, None).await?,
        );

        let llm_client = crate::llm::client::WireLlmClient::new(
            Arc::new(tokio::sync::Mutex::new(wire_client)),
            crate::llm::client::LlmClientConfig::default(),
            crate::llm::cost::CostEstimator::default(),
        );

        let classifier: Arc<dyn ClassifierBackend> = Arc::new(ProductionClassifierBackend {
            inner: Arc::new(crate::runtime::classifier::WireLlmClassifierBackend::new(
                Arc::new(llm_client),
            )),
            cache: Mutex::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(1024).unwrap_or(std::num::NonZeroUsize::MIN),
            )),
        });

        let llm_direct_client = crate::llm::client::WireLlmClient::new(
            Arc::new(tokio::sync::Mutex::new(
                crate::wire::ProcessWireClient::new(
                    crate::wire::ChildProcessTransport::spawn("kimi", None, None, None).await?,
                ),
            )),
            crate::llm::client::LlmClientConfig::default(),
            crate::llm::cost::CostEstimator::default(),
        );

        let llm_direct: Arc<dyn LlmDirectBackend> = Arc::new(ProductionLlmDirectBackend {
            inner: Arc::new(tokio::sync::Mutex::new(llm_direct_client)),
        });

        let wire_worker: Arc<dyn WireWorkerBackend> = Arc::new(PlaceholderWireWorkerBackend);
        let goal_bridge: Arc<dyn GoalBridgeBackend> = Arc::new(ProductionGoalBridgeBackend);

        let router = Arc::new(Router::new(
            classifier.clone(),
            llm_direct,
            wire_worker,
            goal_bridge,
            RouterConfig::default(),
            event_bus.clone(),
        ));

        Ok(Self {
            router,
            classifier,
            session,
            event_bus,
            latest_goal_id: Mutex::new(None),
        })
    }

    pub fn event_bus(&self) -> Arc<EventBus> {
        self.event_bus.clone()
    }
}

#[async_trait]
impl CommandBackend for ProductionBackend {
    async fn dispatch_quick(&self, prompt: &str) -> CommandResponse {
        let outcome = self
            .router
            .dispatch_with_intent_override(prompt, Intent::Small, &self.session)
            .await;
        match outcome {
            RouteOutcome::Small { .. } => {
                CommandResponse::Text("small edit dispatched".to_string())
            }
            RouteOutcome::Refused { reason } => CommandResponse::Error(reason),
            other => CommandResponse::Text(format!("{:?}", other)),
        }
    }

    async fn dispatch_escalate(&self, prompt: &str) -> CommandResponse {
        let outcome = self
            .router
            .dispatch_with_intent_override(prompt, Intent::Large, &self.session)
            .await;
        match &outcome {
            RouteOutcome::Large { goal_id, .. } => {
                *self.latest_goal_id.lock().await = Some(goal_id.clone());
                CommandResponse::Text(format!("large goal created: {}", goal_id))
            }
            RouteOutcome::Refused { reason } => CommandResponse::Error(reason.clone()),
            other => CommandResponse::Text(format!("{:?}", other)),
        }
    }

    async fn dispatch_classify(&self, prompt: &str) -> CommandResponse {
        let input = ClassifierInput {
            prompt: prompt.to_string(),
            recent_conversation: vec![],
            project_root: self.session.project_root.clone(),
        };
        let output = self.classifier.classify(input).await;
        CommandResponse::Text(format!(
            "intent={:?} confidence={:.2} reasoning={}",
            output.intent, output.confidence, output.reasoning
        ))
    }

    async fn dispatch_explain(&self) -> CommandResponse {
        let path = crate::runtime::classifier::telemetry::telemetry_path();
        match tokio::fs::read_to_string(&path).await {
            Ok(contents) => {
                let last = contents.lines().rfind(|l| !l.trim().is_empty());
                match last {
                    Some(line) => CommandResponse::Text(format!("last telemetry: {}", line)),
                    None => CommandResponse::Text("no telemetry yet".to_string()),
                }
            }
            Err(_) => CommandResponse::Text("no telemetry yet".to_string()),
        }
    }

    async fn dispatch_show_plan(&self) -> CommandResponse {
        let id = self.latest_goal_id.lock().await.clone();
        match id {
            Some(id) => match crate::runtime::goal::chat_api::commands::show_plan(&id).await {
                Ok(plan) => CommandResponse::Markdown(plan),
                Err(e) => CommandResponse::Error(format!("failed to load plan: {}", e)),
            },
            None => CommandResponse::Error("no active goal".to_string()),
        }
    }

    async fn dispatch_show_proof(&self) -> CommandResponse {
        let id = self.latest_goal_id.lock().await.clone();
        match id {
            Some(id) => match crate::runtime::goal::chat_api::commands::show_proof(&id).await {
                Ok(path) => CommandResponse::Text(format!("proof: {}", path.display())),
                Err(e) => CommandResponse::Error(format!("failed to load proof: {}", e)),
            },
            None => CommandResponse::Error("no active goal".to_string()),
        }
    }

    async fn dispatch_show_goals(&self) -> CommandResponse {
        match crate::runtime::goal::chat_api::commands::show_goals(&self.session.session_id).await {
            Ok(list) => {
                if list.is_empty() {
                    CommandResponse::Text("no goals for this session".to_string())
                } else {
                    let lines: Vec<String> = list
                        .iter()
                        .map(|g| format!("- {} ({})", g.goal_id, g.status))
                        .collect();
                    CommandResponse::Text(lines.join("\n"))
                }
            }
            Err(e) => CommandResponse::Error(format!("failed to list goals: {}", e)),
        }
    }

    async fn dispatch_goal_show(&self, goal_id: &str) -> CommandResponse {
        match crate::runtime::goal::chat_api::commands::show_plan(goal_id).await {
            Ok(plan) => CommandResponse::Markdown(plan),
            Err(e) => CommandResponse::Error(format!("failed to load goal plan: {}", e)),
        }
    }

    async fn dispatch_inject(&self, text: &str) -> CommandResponse {
        let id = self.latest_goal_id.lock().await.clone();
        match id {
            Some(id) => match crate::runtime::goal::chat_api::inject_hint(&id, text) {
                Ok(()) => CommandResponse::Ok,
                Err(e) => CommandResponse::Error(format!("inject failed: {}", e)),
            },
            None => CommandResponse::Error("no active goal".to_string()),
        }
    }

    async fn dispatch_pause(&self) -> CommandResponse {
        let id = self.latest_goal_id.lock().await.clone();
        match id {
            Some(id) => match crate::runtime::goal::chat_api::pause(&id).await {
                Ok(()) => CommandResponse::Ok,
                Err(e) => CommandResponse::Error(format!("pause failed: {}", e)),
            },
            None => CommandResponse::Error("no active goal".to_string()),
        }
    }

    async fn dispatch_resume(&self) -> CommandResponse {
        let id = self.latest_goal_id.lock().await.clone();
        match id {
            Some(id) => match crate::runtime::goal::chat_api::resume(&id).await {
                Ok(()) => CommandResponse::Ok,
                Err(e) => CommandResponse::Error(format!("resume failed: {}", e)),
            },
            None => CommandResponse::Error("no active goal".to_string()),
        }
    }

    async fn dispatch_cancel(&self) -> CommandResponse {
        let id = self.latest_goal_id.lock().await.clone();
        match id {
            Some(id) => match crate::runtime::goal::chat_api::cancel(&id).await {
                Ok(()) => {
                    *self.latest_goal_id.lock().await = None;
                    CommandResponse::Ok
                }
                Err(e) => CommandResponse::Error(format!("cancel failed: {}", e)),
            },
            None => CommandResponse::Error("no active goal".to_string()),
        }
    }

    async fn dispatch_approve(&self) -> CommandResponse {
        let id = self.latest_goal_id.lock().await.clone();
        match id {
            Some(id) => match crate::runtime::goal::chat_api::commands::approve_slice(&id).await {
                Ok(()) => CommandResponse::Ok,
                Err(e) => CommandResponse::Error(format!("approve failed: {}", e)),
            },
            None => CommandResponse::Error("no active goal".to_string()),
        }
    }

    async fn dispatch_reject(&self, reason: Option<&str>) -> CommandResponse {
        let id = self.latest_goal_id.lock().await.clone();
        match id {
            Some(id) => {
                match crate::runtime::goal::chat_api::commands::reject_slice(&id, reason).await {
                    Ok(()) => CommandResponse::Ok,
                    Err(e) => CommandResponse::Error(format!("reject failed: {}", e)),
                }
            }
            None => CommandResponse::Error("no active goal".to_string()),
        }
    }

    async fn dispatch_diff(&self) -> CommandResponse {
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new("git")
                .args(["diff"])
                .current_dir(&self.session.project_root)
                .kill_on_drop(true)
                .output(),
        )
        .await;

        match output {
            Ok(Ok(out)) => {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                CommandResponse::Markdown(format!("```diff\n{}\n```", text))
            }
            Ok(Err(e)) => CommandResponse::Error(format!("git diff failed: {}", e)),
            Err(_) => CommandResponse::Error("git diff timed out".to_string()),
        }
    }

    async fn dispatch_cost(&self) -> CommandResponse {
        let cost = *self.session.cumulative_cost_usd.lock().await;
        CommandResponse::Text(format!("session cost: ${:.4}", cost))
    }

    async fn dispatch_new_session(&self) -> CommandResponse {
        CommandResponse::EffectStartNewSession
    }

    async fn dispatch_list_sessions(&self) -> CommandResponse {
        let sessions_dir = crate::runtime::config::state_dir().join("sessions");
        match tokio::fs::read_dir(&sessions_dir).await {
            Ok(mut entries) => {
                let mut names = Vec::new();
                while let Ok(Some(entry)) = entries.next_entry().await {
                    names.push(entry.file_name().to_string_lossy().to_string());
                }
                if names.is_empty() {
                    CommandResponse::Text("no sessions found".to_string())
                } else {
                    CommandResponse::Text(names.join("\n"))
                }
            }
            Err(_) => CommandResponse::Text("no sessions found".to_string()),
        }
    }

    async fn dispatch_resume_session(&self, session_id: &str) -> CommandResponse {
        let sanitized = match crate::runtime::sanitize::sanitize_name(session_id) {
            Ok(s) => s,
            Err(_) => return CommandResponse::Error("invalid session id".to_string()),
        };
        let sessions_dir = crate::runtime::config::state_dir()
            .join("sessions")
            .join(&sanitized);
        if sessions_dir.exists() {
            CommandResponse::EffectStartNewSession
        } else {
            CommandResponse::Error(format!("session {} not found", sanitized))
        }
    }
}
