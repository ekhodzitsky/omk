use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use omk::cli::chat::commands::backend::{CommandBackend, CommandResponse};
use omk::cli::chat::commands::completions::{complete, completions_for_show};
use omk::cli::chat::commands::dispatch::{CommandDispatcher, CommandSessionState, InputDecision};
use omk::cli::chat::commands::help::{render_help_grouped, render_help_table};
use omk::cli::chat::commands::parser::{parse_command, Command, ParseError};
use omk::cli::chat::commands::registry::{find_spec, COMMAND_REGISTRY};

// ===================================================================
// Mock backend that records every call.
// ===================================================================

#[derive(Debug, Default)]
struct MockBackend {
    calls: Mutex<Vec<String>>,
}

impl MockBackend {
    fn record(&self, call: &str) {
        self.calls.lock().unwrap().push(call.to_string());
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl CommandBackend for MockBackend {
    async fn dispatch_quick(&self, prompt: &str) -> CommandResponse {
        self.record(&format!("quick:{prompt}"));
        CommandResponse::Ok
    }
    async fn dispatch_escalate(&self, prompt: &str) -> CommandResponse {
        self.record(&format!("escalate:{prompt}"));
        CommandResponse::Ok
    }
    async fn dispatch_classify(&self, prompt: &str) -> CommandResponse {
        self.record(&format!("classify:{prompt}"));
        CommandResponse::Ok
    }
    async fn dispatch_explain(&self) -> CommandResponse {
        self.record("explain");
        CommandResponse::Ok
    }
    async fn dispatch_show_plan(&self) -> CommandResponse {
        self.record("show_plan");
        CommandResponse::Ok
    }
    async fn dispatch_show_proof(&self) -> CommandResponse {
        self.record("show_proof");
        CommandResponse::Ok
    }
    async fn dispatch_show_goals(&self) -> CommandResponse {
        self.record("show_goals");
        CommandResponse::Ok
    }
    async fn dispatch_goal_show(&self, goal_id: &str) -> CommandResponse {
        self.record(&format!("goal_show:{goal_id}"));
        CommandResponse::Ok
    }
    async fn dispatch_inject(&self, text: &str) -> CommandResponse {
        self.record(&format!("inject:{text}"));
        CommandResponse::Ok
    }
    async fn dispatch_pause(&self) -> CommandResponse {
        self.record("pause");
        CommandResponse::Ok
    }
    async fn dispatch_resume(&self) -> CommandResponse {
        self.record("resume");
        CommandResponse::Ok
    }
    async fn dispatch_cancel(&self) -> CommandResponse {
        self.record("cancel");
        CommandResponse::Ok
    }
    async fn dispatch_approve(&self) -> CommandResponse {
        self.record("approve");
        CommandResponse::Ok
    }
    async fn dispatch_reject(&self, reason: Option<&str>) -> CommandResponse {
        self.record(&format!("reject:{:?}", reason));
        CommandResponse::Ok
    }
    async fn dispatch_diff(&self) -> CommandResponse {
        self.record("diff");
        CommandResponse::Ok
    }
    async fn dispatch_cost(&self) -> CommandResponse {
        self.record("cost");
        CommandResponse::Ok
    }
    async fn dispatch_new_session(&self) -> CommandResponse {
        self.record("new_session");
        CommandResponse::Ok
    }
    async fn dispatch_list_sessions(&self) -> CommandResponse {
        self.record("list_sessions");
        CommandResponse::Ok
    }
    async fn dispatch_resume_session(&self, session_id: &str) -> CommandResponse {
        self.record(&format!("resume_session:{session_id}"));
        CommandResponse::Ok
    }
}

// ===================================================================
// Helpers
// ===================================================================

fn make_dispatcher() -> (CommandDispatcher, Arc<MockBackend>) {
    let backend = Arc::new(MockBackend::default());
    let session = Arc::new(CommandSessionState::new());
    let dispatcher = CommandDispatcher::new(backend.clone(), session);
    (dispatcher, backend)
}

// ===================================================================
// Parser tests
// ===================================================================

#[test]
fn test_parse_command_simple() {
    let cmd = parse_command("/help").unwrap();
    assert_eq!(cmd.name, "help");
    assert!(cmd.args.is_empty());
    assert_eq!(cmd.raw_args, "");
}

#[test]
fn test_parse_command_with_args() {
    let cmd = parse_command("/quick rewrite auth flow").unwrap();
    assert_eq!(cmd.name, "quick");
    assert_eq!(cmd.args, vec!["rewrite", "auth", "flow"]);
    assert_eq!(cmd.raw_args, "rewrite auth flow");
}

#[test]
fn test_parse_command_with_quoted_arg() {
    let cmd = parse_command("/inject \"say hello world\"").unwrap();
    assert_eq!(cmd.name, "inject");
    assert_eq!(cmd.args, vec!["say hello world"]);
    assert_eq!(cmd.raw_args, "\"say hello world\"");
}

#[test]
fn test_parse_command_with_unterminated_quote_errors() {
    let err = parse_command("/inject \"say hello").unwrap_err();
    assert!(matches!(err, ParseError::UnterminatedQuote));
}

#[test]
fn test_parse_command_empty_after_slash_returns_error() {
    assert!(matches!(
        parse_command("/"),
        Err(ParseError::EmptyCommandName)
    ));
    assert!(matches!(
        parse_command("/   "),
        Err(ParseError::EmptyCommandName)
    ));
}

#[test]
fn test_parse_command_non_slash_input_errors() {
    assert!(matches!(
        parse_command("hello"),
        Err(ParseError::NotACommand)
    ));
}

// ===================================================================
// Completion tests
// ===================================================================

#[test]
fn test_tab_completion_for_partial_command() {
    assert_eq!(complete("/he"), vec!["help"]);
    let mut q = complete("/q");
    q.sort_unstable();
    assert_eq!(q, vec!["quick", "quit"]);
    let all = complete("/");
    assert!(all.contains(&"help"));
    assert!(all.contains(&"quick"));
    assert!(all.contains(&"quit"));
    assert!(all.contains(&"?")); // alias
}

#[test]
fn test_tab_completion_for_show_subcommand() {
    assert_eq!(completions_for_show("p"), vec!["plan", "proof"]);
}

// ===================================================================
// Help renderer tests
// ===================================================================

#[test]
fn test_help_renders_all_registry_entries() {
    let table = render_help_table();
    let names: Vec<_> = COMMAND_REGISTRY.iter().map(|s| s.name).collect();
    for name in &names {
        assert!(
            table.contains(&format!("| /{name} ")),
            "help table missing /{name}"
        );
    }
}

#[test]
fn test_help_grouped_contains_categories() {
    let grouped = render_help_grouped();
    assert!(grouped.contains("## Help"));
    assert!(grouped.contains("## Quit"));
    assert!(grouped.contains("/quit"));
}

// ===================================================================
// Dispatch tests
// ===================================================================

#[tokio::test]
async fn test_dispatch_known_command_to_backend() {
    let (disp, mock) = make_dispatcher();
    let resp = disp
        .dispatch(Command {
            name: "quick".to_string(),
            args: vec!["rewrite".to_string()],
            raw_args: "rewrite".to_string(),
        })
        .await;
    assert!(matches!(resp, CommandResponse::Ok));
    assert_eq!(mock.calls(), vec!["quick:rewrite"]);
}

#[tokio::test]
async fn test_unknown_command_first_time_emits_hint() {
    let (disp, _mock) = make_dispatcher();
    let decision = disp.route_input("/notacommand foo").await;
    match decision {
        InputDecision::EmitHintThenSendAsText(hint, text) => {
            assert!(hint.contains("not a command"));
            assert_eq!(text, "/notacommand foo");
        }
        other => panic!("expected EmitHintThenSendAsText, got {other:?}"),
    }
}

#[tokio::test]
async fn test_unknown_command_second_time_is_silent() {
    let (disp, _mock) = make_dispatcher();
    // First time sets the flag.
    let _ = disp.route_input("/notacommand").await;
    // Second time should be silent.
    let decision = disp.route_input("/another").await;
    assert!(matches!(decision, InputDecision::SendAsText(t) if t == "/another"));
}

#[tokio::test]
async fn test_non_command_input_is_text() {
    let (disp, _mock) = make_dispatcher();
    let decision = disp.route_input("just text").await;
    assert!(matches!(decision, InputDecision::SendAsText(t) if t == "just text"));
}

#[tokio::test]
async fn test_resume_with_arg_means_session_resume_without_means_unpause() {
    let (disp, mock) = make_dispatcher();

    // no arg → unpause (dispatch_resume)
    let _ = disp
        .dispatch(Command {
            name: "resume".to_string(),
            args: vec![],
            raw_args: "".to_string(),
        })
        .await;

    // with arg → resume_session
    let _ = disp
        .dispatch(Command {
            name: "resume".to_string(),
            args: vec!["o7k_a8f2".to_string()],
            raw_args: "o7k_a8f2".to_string(),
        })
        .await;

    let calls = mock.calls();
    assert!(calls.contains(&"resume".to_string()));
    assert!(calls.contains(&"resume_session:o7k_a8f2".to_string()));
}

#[tokio::test]
async fn test_dispatch_help_returns_markdown() {
    let (disp, _mock) = make_dispatcher();
    let resp = disp
        .dispatch(Command {
            name: "help".to_string(),
            args: vec![],
            raw_args: "".to_string(),
        })
        .await;
    assert!(matches!(resp, CommandResponse::Markdown(_)));
}

#[tokio::test]
async fn test_dispatch_theme_dark_returns_effect() {
    let (disp, _mock) = make_dispatcher();
    let resp = disp
        .dispatch(Command {
            name: "theme".to_string(),
            args: vec!["dark".to_string()],
            raw_args: "dark".to_string(),
        })
        .await;
    assert!(matches!(resp, CommandResponse::EffectThemeDark));
}

#[tokio::test]
async fn test_dispatch_quit_returns_effect_exit() {
    let (disp, _mock) = make_dispatcher();
    let resp = disp
        .dispatch(Command {
            name: "quit".to_string(),
            args: vec![],
            raw_args: "".to_string(),
        })
        .await;
    assert!(matches!(resp, CommandResponse::EffectExit));
}

#[tokio::test]
async fn test_dispatch_reject_without_reason() {
    let (disp, mock) = make_dispatcher();
    let resp = disp
        .dispatch(Command {
            name: "reject".to_string(),
            args: vec![],
            raw_args: "".to_string(),
        })
        .await;
    assert!(matches!(resp, CommandResponse::Ok));
    assert!(mock.calls()[0].starts_with("reject:"));
}

#[test]
fn test_registry_find_spec_by_alias() {
    assert_eq!(find_spec("?").unwrap().name, "help");
    assert_eq!(find_spec("exit").unwrap().name, "quit");
}

#[test]
fn test_stub_backend_returns_not_wired() {
    let backend = omk::cli::chat::commands::backend::StubBackend;
    let rt = tokio::runtime::Runtime::new().unwrap();
    let resp = rt.block_on(async {
        omk::cli::chat::commands::backend::CommandBackend::dispatch_quick(&backend, "hi").await
    });
    assert!(matches!(
        resp,
        CommandResponse::NotWired {
            reason: "requires W3 router; not yet merged"
        }
    ));
}
