use omk::cli::chat::app::{App, PaneState};
use omk::cli::chat::input::{ChatEvent, KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;

fn key(c: char) -> ChatEvent {
    ChatEvent::Key(KeyEvent {
        code: KeyCode::Char(c),
        modifiers: KeyModifiers::none(),
    })
}

fn enter() -> ChatEvent {
    ChatEvent::Key(KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::none(),
    })
}

fn tab() -> ChatEvent {
    ChatEvent::Key(KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::none(),
    })
}

fn shift_tab() -> ChatEvent {
    ChatEvent::Key(KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers {
            shift: true,
            ..KeyModifiers::none()
        },
    })
}

fn make_app(temp: &TempDir) -> App {
    let state_dir = temp.path().join("state");
    let config_dir = temp.path().join("config");
    App::with_dirs(
        state_dir,
        config_dir,
        "/tmp/project".to_string(),
        "o7k_test1234".to_string(),
    )
    .unwrap()
}

#[test]
fn test_app_starts_with_collapsed_engine_pane() {
    let temp = TempDir::new().unwrap();
    let app = make_app(&temp);
    assert_eq!(app.pane_state, PaneState::Collapsed);
}

#[test]
fn test_tab_expands_engine_pane_then_back_to_compact() {
    let temp = TempDir::new().unwrap();
    let mut app = make_app(&temp);
    assert_eq!(app.pane_state, PaneState::Collapsed);

    app.handle_event(tab());
    assert_eq!(app.pane_state, PaneState::Expanded);

    app.handle_event(tab());
    assert_eq!(app.pane_state, PaneState::Compact);

    app.handle_event(tab());
    assert_eq!(app.pane_state, PaneState::Expanded);
}

#[test]
fn test_shift_tab_collapses_engine_pane() {
    let temp = TempDir::new().unwrap();
    let mut app = make_app(&temp);

    // Get to Compact first (Collapsed -> Tab -> Expanded -> Tab -> Compact)
    app.handle_event(tab()); // Expanded
    app.handle_event(tab()); // Compact
    assert_eq!(app.pane_state, PaneState::Compact);

    app.handle_event(shift_tab());
    assert_eq!(app.pane_state, PaneState::Collapsed);
}

#[test]
fn test_text_input_appends_to_conversation() {
    let temp = TempDir::new().unwrap();
    let mut app = make_app(&temp);

    app.handle_event(key('h'));
    app.handle_event(key('i'));
    app.handle_event(enter());

    let conv_path = app.state_dir.join("conversation.jsonl");
    let contents = std::fs::read_to_string(&conv_path).unwrap();

    assert!(
        contents.contains(r#""role":"user""#),
        "expected user message in conversation.jsonl"
    );
    assert!(
        contents.contains(r#""text":"hi""#),
        "expected 'hi' text in conversation.jsonl"
    );
    assert!(
        contents.contains(r#"[W1 stub] received \"hi\""#),
        "expected stub echo in conversation.jsonl"
    );
}

#[test]
fn test_session_resume_loads_persisted_conversation() {
    let temp = TempDir::new().unwrap();
    let state_dir = temp.path().join("state");
    let config_dir = temp.path().join("config");

    // First session: send 3 prompts.
    {
        let mut app = App::with_dirs(
            state_dir.clone(),
            config_dir.clone(),
            "/tmp/project".to_string(),
            "o7k_test1234".to_string(),
        )
        .unwrap();

        for text in &["one", "two", "three"] {
            for c in text.chars() {
                app.handle_event(key(c));
            }
            app.handle_event(enter());
        }

        let msgs = app.session.conversation.read_all().unwrap();
        assert_eq!(msgs.len(), 6, "expected 6 messages before drop");
    }

    // Resume session with same id.
    {
        let app = App::with_dirs(
            state_dir,
            config_dir,
            "/tmp/project".to_string(),
            "o7k_test1234".to_string(),
        )
        .unwrap();

        let msgs = app.session.conversation.read_all().unwrap();
        assert_eq!(
            msgs.len(),
            6,
            "expected 6 messages after resume (3 user + 3 assistant)"
        );
    }
}

#[test]
#[ignore = "blocked on src/main.rs wiring by orchestrator"]
fn smoke_omk_new_starts_chat_shell() {
    // This test requires `omk` binary to be wired to `run_chat(ChatArgs)`.
    // Once wired, it should spawn `omk --new` and verify the TUI initializes.
    use std::process::Command;
    let output = Command::new("cargo")
        .args(["run", "--", "--new"])
        .output()
        .expect("cargo run should succeed");
    assert!(output.status.success(), "omk --new should exit cleanly");
}
