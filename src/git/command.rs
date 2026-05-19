use crate::git::error::GitError;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::sleep;
use tracing::{debug, warn};

const GIT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 3;

/// Output of a finished git command.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: ExitStatus,
}

/// Internal wrapper around git CLI with timeout, retry and non-interactive env.
#[derive(Debug, Clone)]
pub struct GitCommand {
    repo: PathBuf,
    git_bin: PathBuf,
    timeout: Duration,
    max_retries: u32,
}

impl GitCommand {
    pub fn new(repo: PathBuf) -> Result<Self, GitError> {
        let git_bin = which::which("git").map_err(|_| GitError::GitNotFound)?;
        Ok(Self {
            repo,
            git_bin,
            timeout: GIT_TIMEOUT,
            max_retries: MAX_RETRIES,
        })
    }

    pub async fn run(&self, args: &[impl AsRef<OsStr>]) -> Result<CommandOutput, GitError> {
        self.run_with_env(args, &[]).await
    }

    pub async fn run_with_env(
        &self,
        args: &[impl AsRef<OsStr>],
        extra_env: &[(&str, &str)],
    ) -> Result<CommandOutput, GitError> {
        let command_str = args
            .iter()
            .map(|a| a.as_ref().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let mut attempt = 0;

        loop {
            let mut cmd = Command::new(&self.git_bin);
            cmd.args(args)
                .current_dir(&self.repo)
                .env("GIT_TERMINAL_PROMPT", "0")
                .env("GIT_ASKPASS", "echo")
                .env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes")
                .env("LC_ALL", "C")
                .kill_on_drop(true);

            for (k, v) in extra_env {
                cmd.env(k, v);
            }

            debug!(command = %command_str, attempt, "spawning git command");

            let result = tokio::time::timeout(self.timeout, cmd.output()).await;

            match result {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                    let status = output.status;

                    if status.success() {
                        return Ok(CommandOutput {
                            stdout,
                            stderr,
                            status,
                        });
                    }

                    let exit_code = status.code().unwrap_or(-1);
                    if attempt < self.max_retries && is_retryable(&stderr) {
                        attempt += 1;
                        let backoff = Duration::from_millis(100 * 2_u64.pow(attempt - 1));
                        warn!(
                            command = %command_str,
                            attempt,
                            exit_code,
                            ?backoff,
                            stderr,
                            "git command failed with retryable error, backing off"
                        );
                        sleep(backoff).await;
                        continue;
                    }

                    return Err(GitError::CommandFailed {
                        command: command_str,
                        exit_code,
                        stderr,
                        stdout,
                    });
                }
                Ok(Err(e)) => {
                    return Err(GitError::Io(e.to_string()));
                }
                Err(_) => {
                    return Err(GitError::Timeout(self.timeout, command_str));
                }
            }
        }
    }
}

fn is_retryable(stderr: &str) -> bool {
    let needle = stderr.to_lowercase();
    needle.contains("unable to access")
        || needle.contains("timeout")
        || needle.contains("early eof")
        || needle.contains("fatal: unable to access")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_true() {
        assert!(is_retryable(
            "fatal: unable to access 'https://...': early EOF"
        ));
        assert!(is_retryable("network timeout"));
        assert!(is_retryable("unable to access"));
    }

    #[test]
    fn test_is_retryable_false() {
        assert!(!is_retryable("fatal: not a git repository"));
        assert!(!is_retryable("error: pathspec 'foo' did not match"));
    }
}
