use crate::kimi_native::diagnostics::{DiagResult, Severity};

pub(super) async fn check_kimi_cli(results: &mut Vec<DiagResult>) {
    // Check for Kimi CLI (L1-031)
    match which::which("kimi") {
        Ok(path) => {
            match tokio::process::Command::new("kimi")
                .arg("--version")
                .output()
                .await
            {
                Ok(output) if output.status.success() => {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    results.push(DiagResult {
                        severity: Severity::Ok,
                        message: format!("Kimi CLI {} at {}", version, path.display()),
                        fix_hint: None,
                    });
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let details = if stderr.is_empty() {
                        format!("exit status {}", output.status)
                    } else {
                        stderr
                    };
                    let repair = format!(
                        "Run `{0} --version`; if it still fails, reinstall Kimi CLI from https://www.kimi.com/code/docs and re-check with `command -v kimi && kimi --version`",
                        path.display()
                    );
                    results.push(DiagResult {
                        severity: Severity::Warning,
                        message: format!(
                            "Kimi CLI found at {} but version check failed: {}",
                            path.display(),
                            details
                        ),
                        fix_hint: Some(repair),
                    });
                }
                Err(e) => {
                    let repair = format!(
                        "Run `{0} --version`; if it still fails, reinstall Kimi CLI from https://www.kimi.com/code/docs and re-check with `command -v kimi && kimi --version`",
                        path.display()
                    );
                    results.push(DiagResult {
                        severity: Severity::Warning,
                        message: format!(
                            "Kimi CLI found at {} but version check could not run: {}",
                            path.display(),
                            e
                        ),
                        fix_hint: Some(repair),
                    });
                }
            }
        }
        Err(_) => {
            results.push(DiagResult {
                severity: Severity::Error,
                message: "Kimi CLI not found in PATH".to_string(),
                fix_hint: Some(
                    "Install Kimi CLI using https://www.kimi.com/code/docs, then run `command -v kimi && kimi --version`".to_string(),
                ),
            });
        }
    }
}
