//! Eager validation for `omk goal` flag values.
//!
//! Validators run before any runtime side effects, so a malformed budget or
//! empty goal text fails fast with an actionable error and never creates a
//! half-written goal scaffold on disk.
//!
//! Note: the runtime (`src/runtime/goal/budget.rs`) re-checks token/USD/time
//! invariants for defense in depth and for library callers that bypass this
//! CLI. Error wording is intentionally CLI-shaped here.

use anyhow::{Context, Result};

use super::OutputFormat;

pub(super) fn resolve_format(format: OutputFormat, json: bool) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else {
        format
    }
}

pub(super) fn validate_goal_text(goal: &str) -> Result<&str> {
    let trimmed = goal.trim();
    if trimmed.is_empty() {
        anyhow::bail!(
            "goal text cannot be empty.\n\nExample:\n  omk goal run \"Fix all clippy warnings\""
        );
    }
    Ok(trimmed)
}

pub(super) fn validate_goal_id(goal_id: &str) -> Result<&str> {
    let trimmed = goal_id.trim();
    if trimmed.is_empty() {
        anyhow::bail!(
            "goal id cannot be empty.\n\nUse `latest` or a concrete goal id (see `omk goal list`)."
        );
    }
    Ok(trimmed)
}

pub(super) fn validate_decision_text<'a>(value: &'a str, flag: &str) -> Result<&'a str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("{flag} cannot be empty.");
    }
    Ok(trimmed)
}

/// Validate an optional duration flag.
///
/// `require_positive=true` rejects `0`/`0s` (used by `budget-add --time`,
/// where adding zero is a no-op). `require_positive=false` accepts `0s` so
/// that `omk goal run --budget-time 0s` can create an already-exhausted
/// goal, matching the runtime's exhaustion semantics.
pub(super) fn validate_budget_time(
    value: Option<&str>,
    flag: &str,
    require_positive: bool,
) -> Result<Option<String>> {
    let Some(raw) = value else { return Ok(None) };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        anyhow::bail!("{flag} cannot be empty.\n\nExample: {flag} 1h");
    }
    let secs = crate::runtime::goal::parse_budget_duration(trimmed)
        .with_context(|| format!("invalid {flag}"))?;
    if require_positive && secs == 0 {
        anyhow::bail!("{flag} must be greater than zero (for example: {flag} 1h).");
    }
    Ok(Some(trimmed.to_string()))
}

pub(super) fn validate_optional_budget_tokens(value: Option<u64>) -> Result<()> {
    if let Some(value) = value {
        if value == 0 {
            anyhow::bail!("--budget-tokens must be greater than zero.");
        }
    }
    Ok(())
}

pub(super) fn validate_optional_budget_usd(value: Option<f64>) -> Result<()> {
    if let Some(value) = value {
        if !value.is_finite() || value <= 0.0 {
            anyhow::bail!(
                "--budget-usd must be a positive, finite number (for example: --budget-usd 5)."
            );
        }
    }
    Ok(())
}

pub(super) fn validate_optional_max_agents(value: Option<usize>) -> Result<()> {
    if let Some(value) = value {
        if value == 0 {
            anyhow::bail!("--max-agents must be greater than zero.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_goal_text_is_rejected() {
        assert!(validate_goal_text("").is_err());
        assert!(validate_goal_text("   ").is_err());
    }

    #[test]
    fn goal_text_is_trimmed() {
        assert_eq!(validate_goal_text("  fix it\n").unwrap(), "fix it");
    }

    #[test]
    fn empty_goal_id_is_rejected() {
        assert!(validate_goal_id("").is_err());
        assert!(validate_goal_id("   ").is_err());
    }

    #[test]
    fn budget_time_accepts_zero_when_not_strict() {
        let value = validate_budget_time(Some("0s"), "--budget-time", false).unwrap();
        assert_eq!(value.as_deref(), Some("0s"));
    }

    #[test]
    fn budget_time_rejects_zero_when_strict() {
        let err = validate_budget_time(Some("0s"), "--time", true).unwrap_err();
        assert!(err.to_string().contains("--time must be greater than zero"));
    }

    #[test]
    fn budget_time_rejects_garbage_format() {
        let err = validate_budget_time(Some("nope"), "--budget-time", false).unwrap_err();
        let chain = format!("{err:#}");
        assert!(chain.contains("invalid --budget-time"));
        assert!(chain.contains("invalid duration 'nope'"));
    }

    #[test]
    fn budget_time_rejects_empty_string() {
        let err = validate_budget_time(Some("   "), "--budget-time", false).unwrap_err();
        assert!(err.to_string().contains("--budget-time cannot be empty"));
    }

    #[test]
    fn budget_tokens_rejects_zero() {
        assert!(validate_optional_budget_tokens(Some(0)).is_err());
        assert!(validate_optional_budget_tokens(Some(1)).is_ok());
        assert!(validate_optional_budget_tokens(None).is_ok());
    }

    #[test]
    fn budget_usd_rejects_non_positive_and_non_finite() {
        assert!(validate_optional_budget_usd(Some(0.0)).is_err());
        assert!(validate_optional_budget_usd(Some(-1.0)).is_err());
        assert!(validate_optional_budget_usd(Some(f64::NAN)).is_err());
        assert!(validate_optional_budget_usd(Some(f64::INFINITY)).is_err());
        assert!(validate_optional_budget_usd(Some(5.0)).is_ok());
        assert!(validate_optional_budget_usd(None).is_ok());
    }

    #[test]
    fn max_agents_rejects_zero() {
        assert!(validate_optional_max_agents(Some(0)).is_err());
        assert!(validate_optional_max_agents(Some(1)).is_ok());
        assert!(validate_optional_max_agents(None).is_ok());
    }

    #[test]
    fn resolve_format_promotes_json_shortcut() {
        assert!(matches!(
            resolve_format(OutputFormat::Text, true),
            OutputFormat::Json
        ));
        assert!(matches!(
            resolve_format(OutputFormat::Md, false),
            OutputFormat::Md
        ));
    }
}
