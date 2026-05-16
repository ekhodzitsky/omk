use crate::mcp::config::McpConfig;
use crate::mcp::registry::McpRegistry;
use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: McpCommands,
}

#[derive(Subcommand, Debug)]
pub enum McpCommands {
    List,
    Doctor,
    Call {
        server: String,
        tool: String,
        #[arg(default_value = "{}")]
        args: String,
    },
}

pub async fn run(args: Args) -> Result<()> {
    match args.command {
        McpCommands::List => cmd_list().await,
        McpCommands::Doctor => cmd_doctor().await,
        McpCommands::Call {
            server,
            tool,
            args: json_args,
        } => cmd_call(&server, &tool, &json_args).await,
    }
}

async fn cmd_list() -> Result<()> {
    let config_path = McpConfig::default_path();
    println!("MCP config path: {}", config_path.display());
    let config = McpConfig::load(&config_path).await?;
    if config.servers.is_empty() {
        println!("No MCP servers configured.");
        println!("Add servers to {} or .omk/mcp.json", config_path.display());
        return Ok(());
    }
    println!("Configured MCP servers:");
    for (name, server_config) in &config.servers {
        println!(
            "  {}: {} {:?}",
            name, server_config.command, server_config.args
        );
    }
    println!("\nDiscovering tools...");
    let registry = McpRegistry::from_config(&config).await?;
    let tools = registry.all_tools();
    if tools.is_empty() {
        println!("No tools discovered from running servers.");
    } else {
        println!("Available tools:");
        for (server, tool) in tools {
            let desc = tool.description.as_deref().unwrap_or("(no description)");
            println!("  {}::{} — {}", server, tool.name, desc);
        }
    }
    Ok(())
}

async fn cmd_doctor() -> Result<()> {
    let config_path = McpConfig::default_path();
    let config = McpConfig::load(&config_path).await?;
    let mut healthy = 0;
    let mut unhealthy = 0;
    for (name, server_config) in &config.servers {
        print!("Checking {}... ", name);
        match try_start_server(name, server_config).await {
            Ok(tools) => {
                println!("OK ({} tools)", tools.len());
                healthy += 1;
            }
            Err(e) => {
                println!("FAIL: {}", e);
                unhealthy += 1;
            }
        }
    }
    println!("\nMCP doctor: {} healthy, {} unhealthy", healthy, unhealthy);
    if unhealthy > 0 {
        anyhow::bail!("{} MCP server(s) failed health check", unhealthy);
    }
    Ok(())
}

async fn cmd_call(_server_name: &str, tool_name: &str, json_args: &str) -> Result<()> {
    let args: Value =
        serde_json::from_str(json_args).map_err(|e| anyhow::anyhow!("invalid JSON args: {e}"))?;
    let config_path = McpConfig::default_path();
    let config = McpConfig::load(&config_path).await?;
    let mut registry = McpRegistry::from_config(&config).await?;
    let result: serde_json::Value = registry.call_tool(tool_name, args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

async fn try_start_server(
    name: &str,
    config: &crate::mcp::config::McpServerConfig,
) -> Result<Vec<String>> {
    use crate::mcp::client::transport::StdioMcpTransport;
    use crate::mcp::client::McpClient;
    let transport =
        StdioMcpTransport::spawn(name, &config.command, &config.args, &config.env).await?;
    let mut client = McpClient::new(transport, name);
    client.initialize().await?;
    let tools = client.list_tools().await?;
    let names: Vec<String> = tools.into_iter().map(|t| t.name).collect();
    client.shutdown().await?;
    Ok(names)
}
