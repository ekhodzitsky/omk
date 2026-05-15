use super::*;

#[tokio::test]
async fn test_provider_detection() {
    assert!(is_provider_installed("bash").await);
    assert!(!is_provider_installed("definitely_not_a_real_binary_12345").await);
}

#[test]
fn test_artifact_path_generation() {
    let path = artifact_path("claude", "20260507-121530").unwrap();
    let name = path.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("20260507-121530"));
    assert!(name.contains("claude"));
    assert!(name.ends_with(".md"));
}

#[test]
fn test_synthesis_prompt_building() {
    let outputs = vec![
        ("claude".to_string(), "Claude answer".to_string()),
        ("kimi".to_string(), "Kimi answer".to_string()),
    ];
    let prompt = build_synthesis_prompt("What is Rust?", &outputs);
    assert!(prompt.contains("What is Rust?"));
    assert!(prompt.contains("Claude answer"));
    assert!(prompt.contains("Kimi answer"));
    assert!(prompt.contains("synthesize"));
}

#[tokio::test]
async fn test_save_artifact_to() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("artifacts").join("ask");
    let path = save_artifact_to(&base, "claude", "test content", "20260507-121530")
        .await
        .unwrap();
    assert!(path.exists());
    let content = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(content, "test content");
}

#[tokio::test]
async fn test_run_advisor_direct_with_mock() {
    let dir = tempfile::tempdir().unwrap();
    // Use a known provider name so provider_command accepts it.
    let script_path = dir.path().join("kimi");
    tokio::fs::write(&script_path, "#!/bin/bash\necho 'mock output'\n")
        .await
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&script_path)
            .await
            .unwrap()
            .permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&script_path, perms)
            .await
            .unwrap();
    }

    let original_path = std::env::var_os("PATH");
    let mut new_path = std::ffi::OsString::from(dir.path());
    new_path.push(":");
    new_path.push(original_path.clone().unwrap_or_default());
    std::env::set_var("PATH", &new_path);

    let result = run_advisor_direct("kimi", "test prompt", 30).await;

    if let Some(path) = original_path {
        std::env::set_var("PATH", path);
    } else {
        std::env::remove_var("PATH");
    }

    assert!(result.is_ok(), "run_advisor_direct failed: {:?}", result);
    assert_eq!(result.unwrap(), "mock output");
}

#[test]
fn test_provider_command_generation() {
    assert_eq!(provider_command("kimi", "hello").unwrap(), "kimi -p hello");
    assert_eq!(
        provider_command("claude", "it's working").unwrap(),
        "claude -p \"it's working\""
    );
}

#[test]
fn test_is_known_provider() {
    assert!(is_known_provider("kimi"));
    assert!(is_known_provider("claude"));
    assert!(!is_known_provider("gpt4"));
    assert!(!is_known_provider(""));
}

#[tokio::test]
async fn test_run_advisor_direct_rejects_unknown_provider() {
    // The argv-mode runner gates the provider against `ALL_PROVIDERS`
    // before any spawn happens, so an unexpected binary name — even one
    // present on PATH — cannot be invoked through this entry point.
    let cases = ["bash", "rm", "sh", "../etc/passwd", "", "kimi; rm -rf /"];
    for provider in cases {
        let result = run_advisor_direct(provider, "any prompt", 5).await;
        assert!(
            result.is_err(),
            "advisor must refuse unknown provider {provider:?}; got {result:?}",
        );
        let message = format!("{:#}", result.unwrap_err());
        assert!(
            message.contains("Unknown provider"),
            "advisor must short-circuit before spawn for {provider:?}; got error: {message}",
        );
    }
}
