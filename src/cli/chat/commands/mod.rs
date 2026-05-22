pub mod backend;
pub mod completions;
pub mod dispatch;
pub mod help;
pub mod parser;
pub mod registry;

pub use backend::{CommandBackend, CommandResponse, StubBackend};
pub use completions::{complete, completions_for_show};
pub use dispatch::{CommandDispatcher, CommandSessionState, InputDecision};
pub use help::{render_help_grouped, render_help_table};
pub use parser::{parse_command, Command, ParseError};
pub use registry::{find_spec, CommandCategory, CommandSpec, COMMAND_REGISTRY};

// ------------------------------------------------------------------
// Legacy W1 compatibility — retained until coordination day
// ------------------------------------------------------------------

/// Built-in slash commands available in W1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinCommand {
    Quit,
    ThemeDark,
    ThemeLight,
    Help,
    Unknown(String),
}

/// Parse a slash command from user input.
///
/// Returns `None` if the input does not start with '/'.
pub fn parse_slash(input: &str) -> Option<BuiltinCommand> {
    if !input.starts_with('/') {
        return None;
    }
    let trimmed = input.trim();
    let parts: Vec<&str> = trimmed[1..].split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    match parts[0] {
        "quit" => Some(BuiltinCommand::Quit),
        "theme" => {
            if parts.len() > 1 {
                match parts[1] {
                    "dark" => Some(BuiltinCommand::ThemeDark),
                    "light" => Some(BuiltinCommand::ThemeLight),
                    _ => Some(BuiltinCommand::Unknown(trimmed.to_string())),
                }
            } else {
                Some(BuiltinCommand::Unknown(trimmed.to_string()))
            }
        }
        "help" => Some(BuiltinCommand::Help),
        _ => Some(BuiltinCommand::Unknown(trimmed.to_string())),
    }
}

/// Return possible completions for the current slash input.
pub fn tab_complete(input: &str) -> Vec<String> {
    let candidates = ["/quit", "/theme dark", "/theme light", "/help"];
    candidates
        .iter()
        .filter(|c| c.starts_with(input))
        .map(|c| c.to_string())
        .collect()
}

#[cfg(test)]
mod legacy_tests {
    use super::*;

    #[test]
    fn parse_quit() {
        assert_eq!(parse_slash("/quit"), Some(BuiltinCommand::Quit));
    }

    #[test]
    fn parse_theme_dark() {
        assert_eq!(parse_slash("/theme dark"), Some(BuiltinCommand::ThemeDark));
    }

    #[test]
    fn parse_unknown() {
        assert_eq!(
            parse_slash("/foo"),
            Some(BuiltinCommand::Unknown("/foo".to_string()))
        );
    }

    #[test]
    fn parse_not_slash() {
        assert_eq!(parse_slash("hello"), None);
    }

    #[test]
    fn tab_complete_quit() {
        let comps = tab_complete("/qu");
        assert_eq!(comps, vec!["/quit"]);
    }
}
