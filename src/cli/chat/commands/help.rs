use super::registry::{CommandCategory, COMMAND_REGISTRY};

/// Render a flat Markdown-like help table.
pub fn render_help_table() -> String {
    let mut lines = vec![
        "| Command | Args | What it does |".to_string(),
        "|---------|------|--------------|".to_string(),
    ];

    for spec in COMMAND_REGISTRY {
        lines.push(format!(
            "| /{} | {} | {} |",
            spec.name, spec.args_help, spec.help
        ));
    }

    lines.join("\n")
}

/// Render a grouped help view.  Help is shown first, Quit last.
pub fn render_help_grouped() -> String {
    let mut order = vec![
        CommandCategory::Help,
        CommandCategory::Override,
        CommandCategory::Inspection,
        CommandCategory::GoalControl,
        CommandCategory::Session,
        CommandCategory::Theme,
        CommandCategory::Quit,
    ];

    let mut lines: Vec<String> = Vec::new();

    for cat in &mut order {
        let header = match cat {
            CommandCategory::Help => "Help",
            CommandCategory::Override => "Override",
            CommandCategory::Inspection => "Inspection",
            CommandCategory::GoalControl => "Goal Control",
            CommandCategory::Session => "Session",
            CommandCategory::Theme => "Theme",
            CommandCategory::Quit => "Quit",
        };
        let mut first = true;
        for spec in COMMAND_REGISTRY {
            if spec.category != *cat {
                continue;
            }
            if first {
                lines.push(format!("## {}", header));
                first = false;
            }
            lines.push(format!(
                "  /{} {} — {}",
                spec.name, spec.args_help, spec.help
            ));
        }
    }

    lines.join("\n")
}
