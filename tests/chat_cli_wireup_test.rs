use clap::Parser;
use omk::cli::app::{Commands, Omk};
use omk::cli::chat::run::ChatArgs;

#[test]
fn test_omk_no_args_dispatches_to_chat() {
    let omk = Omk::try_parse_from(["omk"]).expect("should parse with no args");
    assert!(
        omk.command.is_none(),
        "expected no subcommand => default to chat"
    );
}

#[test]
fn test_omk_chat_explicit_dispatches_to_chat() {
    let omk = Omk::try_parse_from(["omk", "chat"]).expect("should parse chat subcommand");
    assert!(
        matches!(omk.command, Some(Commands::Chat(_))),
        "expected Commands::Chat"
    );
}

#[test]
fn test_omk_chat_with_session_flag_parses() {
    let omk =
        Omk::try_parse_from(["omk", "chat", "--session", "o7k_abc"]).expect("should parse session");
    match omk.command {
        Some(Commands::Chat(args)) => {
            assert_eq!(args.session, Some("o7k_abc".to_string()));
            assert!(!args.new);
        }
        other => panic!("expected Commands::Chat, got {:?}", other),
    }
}

#[test]
fn test_omk_chat_with_new_flag_parses() {
    let omk = Omk::try_parse_from(["omk", "chat", "--new"]).expect("should parse --new");
    match omk.command {
        Some(Commands::Chat(args)) => {
            assert!(args.new);
            assert!(args.session.is_none());
        }
        other => panic!("expected Commands::Chat, got {:?}", other),
    }
}

#[test]
fn test_existing_subcommand_still_parses() {
    // Goal requires a subcommand; use 'run' as a valid leaf.
    let omk = Omk::try_parse_from(["omk", "goal", "list"]).expect("goal list should still parse");
    assert!(matches!(omk.command, Some(Commands::Goal(_))));

    let omk = Omk::try_parse_from(["omk", "autopilot"]).expect("autopilot should still parse");
    assert!(matches!(omk.command, Some(Commands::Autopilot(_))));
}

#[test]
fn test_chat_args_default_is_no_session_no_new() {
    let args = ChatArgs::default();
    assert!(args.session.is_none());
    assert!(!args.new);
}

#[tokio::test]
async fn test_production_backend_builds_without_panic() {
    let temp = tempfile::tempdir().expect("tempdir");
    let result = omk::cli::chat::composed_backend::ProductionBackend::build(
        "test-session-id".to_string(),
        temp.path().to_path_buf(),
    )
    .await;
    assert!(
        result.is_ok(),
        "ProductionBackend::build should succeed: {:?}",
        result.err()
    );
}
