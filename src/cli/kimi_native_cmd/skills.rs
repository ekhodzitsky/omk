use anyhow::Result;

pub(super) async fn cmd_skills() -> Result<()> {
    let data_dir = crate::runtime::config::data_dir();
    let skills_dir = data_dir.join("skills");

    if !skills_dir.exists() {
        println!("ℹ️  No skills directory found at {}", skills_dir.display());
        println!("   Skills are discovered from .kimi/skills/, .claude/skills/, etc.");
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(&skills_dir).await?;
    let mut skills = vec![];
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            skills.push(name);
        }
    }

    println!("📋 Available Skills ({}):", skills.len());
    for skill in &skills {
        println!("  • {}", skill);
    }
    Ok(())
}
