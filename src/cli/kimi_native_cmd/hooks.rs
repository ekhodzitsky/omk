use anyhow::Result;

pub(super) fn cmd_hooks() -> Result<()> {
    let hooks = crate::kimi_native::hook_spec::default_project_hooks();
    println!("📋 OMK Project Hooks ({}):", hooks.hooks.len());
    for hook in &hooks.hooks {
        println!("  • {:?}", hook.event);
    }
    println!("\n  Scripts ({}):", hooks.scripts.len());
    for (name, _) in &hooks.scripts {
        println!("    • {}", name);
    }
    Ok(())
}
