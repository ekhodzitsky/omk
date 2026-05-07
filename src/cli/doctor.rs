use anyhow::Result;
use clap::Parser;
use std::process::Command;

#[derive(Parser, Debug)]
pub struct Args {}

pub async fn run(_args: Args) -> Result<()> {
    println!("🔍 omk doctor");
    println!();

    let mut issues = 0;

    // Check tmux
    print!("  tmux .................. ");
    match check_cmd("tmux", &["-V"]) {
        Ok(out) => println!("✓ {}", out.trim()),
        Err(e) => {
            println!("✗ {}", e);
            issues += 1;
        }
    }

    // Check kimi
    print!("  kimi CLI .............. ");
    match check_cmd("kimi", &["--version"]) {
        Ok(out) => println!("✓ {}", out.trim()),
        Err(e) => {
            println!("✗ {}", e);
            issues += 1;
        }
    }

    // Check Rust (optional, for dev)
    print!("  Rust .................. ");
    match check_cmd("rustc", &["--version"]) {
        Ok(out) => println!("✓ {}", out.trim()),
        Err(_) => println!("⚠ not installed (optional for dev)"),
    }

    // Check config dirs
    print!("  Config dir ............ ");
    let config_dir = crate::runtime::config::config_dir();
    match check_dir_writable(&config_dir).await {
        Ok(_) => println!("✓ {}", config_dir.display()),
        Err(e) => {
            println!("✗ {}", e);
            issues += 1;
        }
    }

    print!("  State dir ............. ");
    let state_dir = crate::runtime::config::state_dir();
    match check_dir_writable(&state_dir).await {
        Ok(_) => println!("✓ {}", state_dir.display()),
        Err(e) => {
            println!("✗ {}", e);
            issues += 1;
        }
    }

    print!("  Data dir .............. ");
    let data_dir = crate::runtime::config::data_dir();
    match check_dir_writable(&data_dir).await {
        Ok(_) => println!("✓ {}", data_dir.display()),
        Err(e) => {
            println!("✗ {}", e);
            issues += 1;
        }
    }

    print!("  Cache dir ............. ");
    let cache_dir = crate::runtime::config::cache_dir();
    match check_dir_writable(&cache_dir).await {
        Ok(_) => println!("✓ {}", cache_dir.display()),
        Err(e) => {
            println!("✗ {}", e);
            issues += 1;
        }
    }

    // Check bundled skills
    print!("  Bundled skills ........ ");
    let skills_dir = data_dir.join("skills");
    if skills_dir.exists() {
        let count = std::fs::read_dir(&skills_dir)
            .map(|d| d.filter(|e| e.as_ref().map(|e| e.path().is_dir()).unwrap_or(false)).count())
            .unwrap_or(0);
        println!("✓ {} skill(s) in {}", count, skills_dir.display());
    } else {
        println!("⚠ not found. Run `omk setup` to initialize.");
    }

    println!();
    if issues == 0 {
        println!("✅ All checks passed. omk is ready to use.");
    } else {
        println!("⚠️  Found {} issue(s). Please fix them before using omk.", issues);
    }

    Ok(())
}

fn check_cmd(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| anyhow::anyhow!("{} not found ({e})", cmd))?;

    if !output.status.success() {
        anyhow::bail!("{} exited with error", cmd);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn check_dir_writable(path: &std::path::Path) -> Result<()> {
    tokio::fs::create_dir_all(path).await?;
    let test_file = path.join(".omk-write-test");
    tokio::fs::write(&test_file, b"test").await?;
    tokio::fs::remove_file(&test_file).await?;
    Ok(())
}
