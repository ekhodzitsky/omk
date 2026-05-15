use anyhow::Result;
use tokio::process::Command;

use super::UpdateArgs;

pub(super) async fn run_update(args: UpdateArgs) -> Result<()> {
    use tokio::process::Command;

    let current = env!("CARGO_PKG_VERSION");
    println!("Current version: {current}");

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let target = match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        _ => {
            anyhow::bail!("Unsupported platform: {os} {arch}");
        }
    };

    println!("Checking for latest release...");
    let latest = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        Command::new("curl")
            .args([
                "-fsSL",
                "-H",
                "Accept: application/vnd.github+json",
                "https://api.github.com/repos/ekhodzitsky/oh-my-kimi/releases/latest",
            ])
            .output(),
    )
    .await
    {
        Ok(Ok(out)) if out.status.success() => {
            let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
            json["tag_name"].as_str().unwrap_or("").to_string()
        }
        _ => {
            anyhow::bail!("Failed to check for updates. Are you online?");
        }
    };

    if latest.is_empty() {
        anyhow::bail!("Could not determine latest version");
    }

    let latest_version = latest.trim_start_matches('v');
    println!("Latest version: {latest_version}");

    if latest_version == current {
        println!("✓ You are already on the latest version ({current}).");
        return Ok(());
    }

    if args.check {
        println!("Update available: {current} → {latest_version}");
        println!("Run `omk update` to install.");
        return Ok(());
    }

    let asset = format!("omk-{latest_version}-{target}.tar.gz");
    let base_url = format!("https://github.com/ekhodzitsky/oh-my-kimi/releases/download/{latest}");
    let url = format!("{base_url}/{asset}");
    let sha_url = format!("{base_url}/{asset}.sha256");

    println!("Downloading {url}...");

    let tmp_dir = tempfile::tempdir()?;
    let tar_path = tmp_dir.path().join(&asset);
    let sha_path = tmp_dir.path().join(format!("{asset}.sha256"));

    let download = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        Command::new("curl")
            .args(["-fsSL", "-o"])
            .arg(&tar_path)
            .arg(&url)
            .status(),
    )
    .await??;

    if !download.success() {
        anyhow::bail!("Download failed. Prebuilt binary may not be available for {target}.");
    }

    // SHA256 verification is mandatory. Without it, a MITM on the CDN or a
    // compromised release artifact would land arbitrary code on the host.
    println!("Fetching checksum...");
    let sha_download = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        Command::new("curl")
            .args(["-fsSL", "-o"])
            .arg(&sha_path)
            .arg(&sha_url)
            .status(),
    )
    .await??;

    if !sha_download.success() {
        anyhow::bail!(
            "Checksum file not found at {sha_url}. \
             Refusing to install an unverified binary. \
             Re-run with cargo: cargo install --git https://github.com/ekhodzitsky/oh-my-kimi.git"
        );
    }

    verify_sha256(&tar_path, &sha_path).await?;
    println!("✓ SHA256 verified");

    println!("Extracting...");
    let extract = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        Command::new("tar")
            .args(["--no-same-owner", "-xzf"])
            .arg(&tar_path)
            .arg("-C")
            .arg(tmp_dir.path())
            .status(),
    )
    .await??;

    if !extract.success() {
        anyhow::bail!("Failed to extract archive");
    }

    // Release tarballs are flat (`omk` at the root); the older nested layout
    // (`omk-<ver>-<target>/omk`) is kept as a fallback for transitional
    // versions still in the wild.
    let new_binary = tmp_dir.path().join("omk");
    if !new_binary.exists() {
        let legacy = tmp_dir
            .path()
            .join(format!("omk-{latest_version}-{target}"))
            .join("omk");
        if legacy.exists() {
            tokio::fs::copy(&legacy, &new_binary).await?;
        } else {
            anyhow::bail!("Could not find omk binary in downloaded archive");
        }
    }

    let current_exe = std::env::current_exe()?;
    println!("Replacing {}...", current_exe.display());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&new_binary).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&new_binary, perms).await?;
    }

    // Install atomically: write next to current_exe, fsync, rename. A partial
    // copy mid-write previously left users with a corrupt or zero-byte omk.
    install_binary_atomically(&new_binary, &current_exe).await?;

    println!("✓ Updated to {latest_version}");
    println!("  Binary: {}", current_exe.display());

    println!("Updating shell completions...");
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        Command::new(&current_exe)
            .args(["completions", "bash"])
            .output(),
    )
    .await;
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        Command::new(&current_exe)
            .args(["completions", "zsh"])
            .output(),
    )
    .await;
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        Command::new(&current_exe)
            .args(["completions", "fish"])
            .output(),
    )
    .await;

    println!("Run `omk doctor` to verify the installation.");
    Ok(())
}

/// Verify the SHA256 checksum of `archive_path` against the digest recorded
/// in `sha_path`.
///
/// The sha file is produced by `sha256sum` / `shasum -a 256` and follows the
/// `<hex-digest>  <basename>` format. We shell out to the same tool (either
/// `sha256sum` on Linux or `shasum -a 256` on macOS), running it with `cwd`
/// set to the archive's parent so the relative filename in the sha file
/// resolves correctly. Refusing to install on verification failure is
/// non-optional — the alternative is RCE-on-MITM.
async fn verify_sha256(archive_path: &std::path::Path, sha_path: &std::path::Path) -> Result<()> {
    use anyhow::Context;

    let parent = archive_path.parent().unwrap_or(std::path::Path::new("."));
    let sha_file_name = sha_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("checksum file path has no name"))?;
    let archive_name = archive_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("archive path has no name"))?
        .to_string_lossy()
        .into_owned();

    // Belt-and-braces: assert the .sha256 file actually references our
    // archive's basename, so a copy-paste mistake or a transitional rename
    // (sha file says `omk-0.3.30-...` but we downloaded `omk-0.3.31-...`)
    // fails loudly instead of silently passing if both happen to coexist in
    // the same directory.
    let sha_contents = tokio::fs::read_to_string(sha_path)
        .await
        .with_context(|| format!("failed to read checksum file {}", sha_path.display()))?;
    let mut referenced = false;
    for line in sha_contents.lines() {
        // Format: `<hex-digest>  <filename>` (sha256sum / shasum -a 256).
        if let Some(name) = line.split_whitespace().nth(1) {
            if name == archive_name {
                referenced = true;
                break;
            }
        }
    }
    if !referenced {
        anyhow::bail!(
            "Checksum file {} does not reference {}; refusing to verify against an unrelated digest",
            sha_path.display(),
            archive_name
        );
    }

    for cmd in [
        ("sha256sum", vec!["-c"]),
        ("shasum", vec!["-a", "256", "-c"]),
    ] {
        if which::which(cmd.0).is_err() {
            continue;
        }
        let status = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            Command::new(cmd.0)
                .args(&cmd.1)
                .arg(sha_file_name)
                .current_dir(parent)
                .status(),
        )
        .await
        .context("sha256 verification command timed out")?
        .context("failed to spawn sha256 verification command")?;

        if status.success() {
            return Ok(());
        }
        anyhow::bail!(
            "Checksum mismatch for {}; refusing to install an unverified binary",
            archive_path.display()
        );
    }

    anyhow::bail!(
        "Neither sha256sum nor shasum is installed; cannot verify the download. \
         Install one and re-run, or use `cargo install`."
    );
}

/// Install `new_binary` as `current_exe` atomically.
///
/// Writes to a sibling `.omk.new` first, fsyncs, then renames into place.
/// Without this, a partial `tokio::fs::copy` (ENOSPC, signal, disk error)
/// previously left users with a corrupt or zero-byte omk. `rename` on the
/// same filesystem is atomic on Unix.
async fn install_binary_atomically(
    new_binary: &std::path::Path,
    current_exe: &std::path::Path,
) -> Result<()> {
    use anyhow::Context;

    let install_dir = current_exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("current_exe has no parent directory"))?;

    // Pre-flight check: a non-root user installing into /usr/local/bin will
    // hit EACCES on the rename below, with a downstream error that doesn't
    // point at the underlying permission problem. Catch it here with an
    // actionable message.
    let probe = install_dir.join(".omk.write-probe");
    if let Err(e) = tokio::fs::write(&probe, b"").await {
        anyhow::bail!(
            "No write access to {}: {}. Re-run with sudo, or install to a user-writable \
             location (e.g. cargo install path / ~/.local/bin / Homebrew prefix).",
            install_dir.display(),
            e
        );
    }
    let _ = tokio::fs::remove_file(&probe).await;

    let staging = install_dir.join(".omk.new");

    // Drop any prior staging file from a previous failed run.
    if staging.exists() {
        let _ = tokio::fs::remove_file(&staging).await;
    }

    tokio::fs::copy(new_binary, &staging)
        .await
        .with_context(|| format!("failed to stage new binary at {}", staging.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // 0o755 immediately after copy. Brief umask-default window between
        // copy and permission-set is harmless because (a) staging is in the
        // install dir, not a world-writable temp, and (b) the file is not
        // executable until the chmod completes, but it also isn't yet at
        // current_exe — nothing executes it.
        let mut perms = tokio::fs::metadata(&staging).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&staging, perms).await?;
    }

    // sync_data() flushes the staging file's contents to disk before the
    // rename swap, so a power-fail mid-install cannot leave behind a half-
    // written replacement.
    let staging_for_sync = staging.clone();
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let f = std::fs::OpenOptions::new()
            .read(true)
            .open(&staging_for_sync)?;
        f.sync_data()
    })
    .await
    .context("sync task panicked")?
    .context("failed to fsync staged binary")?;

    tokio::fs::rename(&staging, current_exe)
        .await
        .with_context(|| {
            format!(
                "failed to rename {} into {}",
                staging.display(),
                current_exe.display()
            )
        })?;

    Ok(())
}
