use super::types::GoalState;

pub(crate) fn default_goal_agent_task_budget_secs() -> u64 {
    300
}

pub(crate) fn goal_agent_task_budget_secs(state: &GoalState, requested_secs: u64) -> u64 {
    let Some(total_budget_secs) = state
        .budget_time
        .as_deref()
        .and_then(parse_goal_duration_secs)
    else {
        return requested_secs;
    };
    let per_task_ceiling = if total_budget_secs < 60 {
        total_budget_secs.max(1)
    } else {
        (total_budget_secs / 4).max(60)
    };
    requested_secs.min(per_task_ceiling)
}

pub(crate) fn parse_goal_duration_secs(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (number, multiplier) = match trimmed.chars().last()? {
        's' | 'S' => (&trimmed[..trimmed.len() - 1], 1),
        'm' | 'M' => (&trimmed[..trimmed.len() - 1], 60),
        'h' | 'H' => (&trimmed[..trimmed.len() - 1], 60 * 60),
        'd' | 'D' => (&trimmed[..trimmed.len() - 1], 24 * 60 * 60),
        _ => (trimmed, 1),
    };
    number.trim().parse::<u64>().ok()?.checked_mul(multiplier)
}

/// Validate a budget duration string and return the parsed seconds.
///
/// Accepts non-empty values with optional suffix `s`/`m`/`h`/`d`. The runtime
/// allows `0s` to mean "already exhausted" at goal creation time; callers that
/// require a strictly positive duration should enforce it separately.
pub(crate) fn parse_budget_duration(value: &str) -> anyhow::Result<u64> {
    parse_goal_duration_secs(value).ok_or_else(|| {
        anyhow::anyhow!(
            "invalid duration '{value}': expected a number with optional suffix \
             s/m/h/d (for example: 30s, 15m, 8h, 7d)"
        )
    })
}

pub(crate) fn format_goal_duration_secs(secs: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;

    if secs != 0 && secs % DAY == 0 {
        format!("{}d", secs / DAY)
    } else if secs != 0 && secs % HOUR == 0 {
        format!("{}h", secs / HOUR)
    } else if secs != 0 && secs % MINUTE == 0 {
        format!("{}m", secs / MINUTE)
    } else {
        format!("{secs}s")
    }
}
