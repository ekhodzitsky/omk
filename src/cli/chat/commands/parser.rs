/// Parsed slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub args: Vec<String>,
    pub raw_args: String,
}

/// Errors produced while parsing a slash command.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("input does not start with '/'")]
    NotACommand,
    #[error("empty command after slash")]
    EmptyCommandName,
    #[error("unterminated quoted argument")]
    UnterminatedQuote,
}

/// Parse a slash command from raw input.
///
/// Rules:
/// * Input must start with `'/'`.
/// * The first word (after optional leading whitespace) becomes `name`
///   without the leading slash.
/// * Remaining text is split into `args` using a quote-aware shell-like
///   splitter (`shlex`).
/// * `raw_args` is the substring **after** the command name and the first
///   separating whitespace character, with no trimming.
pub fn parse_command(input: &str) -> Result<Command, ParseError> {
    if !input.starts_with('/') {
        return Err(ParseError::NotACommand);
    }

    // Locate the start of the command name (skip whitespace after '/').
    let name_start = input[1..]
        .char_indices()
        .find(|(_, c)| !c.is_whitespace())
        .map(|(i, _)| 1 + i)
        .unwrap_or(input.len());

    if name_start >= input.len() {
        return Err(ParseError::EmptyCommandName);
    }

    // Locate the end of the command name (first whitespace or end of string).
    let name_end = input[name_start..]
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, _)| name_start + i)
        .unwrap_or(input.len());

    let name = input[name_start..name_end].to_string();

    // `raw_args` is everything after the first whitespace that follows the name.
    let raw_args = if name_end < input.len() {
        let after_name = &input[name_end..];
        let skip = after_name.chars().next().map_or(1, |c| c.len_utf8());
        input[name_end + skip..].to_string()
    } else {
        String::new()
    };

    let args = if raw_args.is_empty() {
        Vec::new()
    } else {
        shlex::split(&raw_args).ok_or(ParseError::UnterminatedQuote)?
    };

    Ok(Command {
        name,
        args,
        raw_args,
    })
}
