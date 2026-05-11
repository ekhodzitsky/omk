use anyhow::Result;
use clap::Parser;
use tokio::process::Command;

#[derive(Parser, Debug)]
pub(crate) struct Args {}

pub(crate) async fn run(_args: Args) -> Result<()> {
    println!("🔍 omk doctor");
    println!();

    let mut issues = 0;

    // Check kimi
    print!("  kimi CLI .............. ");
    match check_cmd("kimi", &["--version"]).await {
        Ok(out) => println!("✓ {}", out.trim()),
        Err(e) => {
            println!("✗ {}", e);
            issues += 1;
        }
    }

    // Check Rust (optional, for dev)
    print!("  Rust .................. ");
    match check_cmd("rustc", &["--version"]).await {
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
        let count = match tokio::fs::read_dir(&skills_dir).await {
            Ok(mut d) => {
                let mut count = 0;
                while let Ok(Some(entry)) = d.next_entry().await {
                    if entry
                        .file_type()
                        .await
                        .map(|ft| ft.is_dir())
                        .unwrap_or(false)
                    {
                        count += 1;
                    }
                }
                count
            }
            Err(_) => 0,
        };
        println!("✓ {} skill(s) in {}", count, skills_dir.display());
    } else {
        println!("⚠ not found. Run `omk setup` to initialize.");
    }

    // Check AGENTS.md
    print!("  AGENTS.md ............. ");
    let project_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let agents_path = project_dir.join(".omk").join("AGENTS.md");
    if agents_path.exists() {
        match tokio::fs::read_to_string(&agents_path).await {
            Ok(content) => match crate::agents::parser::parse_agents_md(&content) {
                Ok(manifest) => {
                    let name = manifest.name.as_deref().unwrap_or("(unnamed)");
                    println!("✓ {} ({} agent(s))", name, manifest.agents.len());
                }
                Err(e) => {
                    println!("⚠ Invalid format: {}", e);
                    issues += 1;
                }
            },
            Err(e) => {
                println!("✗ Cannot read: {}", e);
                issues += 1;
            }
        }
    } else {
        println!("⚠ not found. Run `omk setup` to create.");
    }

    // Check gates
    print!("  Verification gates .... ");
    let gate_config = crate::runtime::gates::load_or_detect_gates(&project_dir).await;
    if gate_config.gates.is_empty() {
        println!("⚠ no gates configured (unknown project type)");
    } else {
        let names: Vec<String> = gate_config.gates.iter().map(|g| g.name.clone()).collect();
        println!("✓ {} gate(s): {}", names.len(), names.join(", "));
    }

    // Check registries
    let config = crate::runtime::config::load_config()
        .await
        .unwrap_or_default();
    if !config.registries.is_empty() {
        println!("  Registries:");
        for url in &config.registries {
            print!("    {} ... ", url);
            let result = if url.starts_with("http://") || url.starts_with("https://") {
                crate::marketplace::MarketplaceRegistry::fetch(url).await
            } else {
                crate::marketplace::MarketplaceRegistry::fetch_file(std::path::Path::new(url)).await
            };
            match result {
                Ok(r) => println!("✓ {} ({} skill(s))", r.name, r.skills.len()),
                Err(e) => {
                    println!("✗ {}", e);
                    issues += 1;
                }
            }
        }
    }

    println!();
    if issues == 0 {
        println!("✅ All checks passed. omk is ready to use.");
    } else {
        println!(
            "⚠️  Found {} issue(s). Please fix them before using omk.",
            issues
        );
    }

    Ok(())
}

async fn check_cmd(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("{} not found ({e})", cmd))?;

    if !output.status.success() {
        anyhow::bail!("{} exited with error", cmd);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn check_dir_writable(path: &std::path::Path) -> Result<()> {
    crate::runtime::config::ensure_private_dir(path).await?;
    let test_file = path.join(".omk-write-test");
    tokio::fs::write(&test_file, b"test").await?;
    tokio::fs::remove_file(&test_file).await?;
    Ok(())
}
