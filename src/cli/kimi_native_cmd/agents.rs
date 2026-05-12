use anyhow::Result;

pub(super) fn cmd_agents() -> Result<()> {
    let agents = crate::kimi_native::agent_spec::default_role_agents();
    println!("📋 OMK Role Agents ({}):", agents.len());
    for agent in &agents {
        println!(
            "  • {} — {}",
            agent.id,
            agent.system_prompt.split('.').next().unwrap_or("")
        );
    }
    Ok(())
}
