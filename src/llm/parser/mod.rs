use serde::Deserialize;
use serde_json::Value;

use super::error::LlmError;
use super::types::{Complexity, Difficulty, GoalClassification, GoalKind, Plan, Slice};

/// Extract JSON from a string that may be wrapped in markdown code fences.
///
/// Handles any language tag after the opening backticks (e.g. ```json, ```javascript)
/// and tries to avoid stripping backticks that are part of the JSON content itself
/// by looking for a closing fence on its own line.
fn extract_json(text: &str) -> Result<&str, LlmError> {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return Ok(trimmed);
    }

    // Strip opening backticks and optional language tag.
    let after_open = &trimmed[3..];
    let after_lang = if let Some(nl) = after_open.find('\n') {
        &after_open[nl + 1..]
    } else {
        // Inline fence: no newline, strip any non-backtick prefix (language tag).
        after_open.trim_start_matches(|c: char| c.is_alphanumeric() || c == '-' || c == '_')
    };

    // Look for a closing fence on its own line (preceded by newline).
    if let Some(pos) = after_lang.rfind("\n```") {
        Ok(after_lang[..pos].trim())
    } else {
        // Fallback: strip trailing ``` if present.
        Ok(after_lang.strip_suffix("```").unwrap_or(after_lang).trim())
    }
}

/// Parse a JSON value, returning a descriptive [`LlmError::ParseError`] on failure.
pub(crate) fn parse_json_value(text: &str) -> Result<Value, LlmError> {
    let json_str = extract_json(text)?;
    serde_json::from_str(json_str).map_err(|e| LlmError::ParseError {
        raw: text.to_string(),
        reason: format!("invalid JSON: {e}"),
    })
}

// ============================================================================
// Plan
// ============================================================================

#[derive(Debug, Deserialize, Default)]
struct SliceJson {
    #[serde(default)]
    id: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    write_set: Vec<String>,
    #[serde(default)]
    estimated_difficulty: String,
}

#[derive(Debug, Deserialize, Default)]
struct PlanJson {
    #[serde(default)]
    goal_text: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    complexity_score: u8,
    #[serde(default)]
    complexity_reasoning: String,
    #[serde(default)]
    estimated_hours: Option<f32>,
    #[serde(default)]
    slices: Vec<SliceJson>,
    #[serde(default)]
    dependencies: Vec<Vec<usize>>,
    #[serde(default)]
    acceptance_criteria: Vec<String>,
    #[serde(default)]
    estimated_tokens: usize,
}

/// Parse a JSON string into a [`Plan`].
///
/// Missing fields use sensible defaults; markdown-wrapped JSON is handled.
pub fn parse_plan_json(goal_text: &str, json: &str) -> Result<Plan, LlmError> {
    let value = parse_json_value(json)?;
    let raw: PlanJson = serde_json::from_value(value).map_err(|e| LlmError::ParseError {
        raw: json.to_string(),
        reason: format!("plan JSON structure mismatch: {e}"),
    })?;

    let kind = parse_goal_kind(&raw.kind);

    let slices: Vec<Slice> = raw
        .slices
        .into_iter()
        .enumerate()
        .map(|(idx, s)| Slice {
            id: if s.id.is_empty() {
                format!("slice-{}", idx)
            } else {
                s.id
            },
            description: s.description,
            write_set: s.write_set,
            estimated_difficulty: parse_difficulty(&s.estimated_difficulty),
        })
        .collect();

    let mut dependencies = Vec::with_capacity(raw.dependencies.len());
    let slice_count = slices.len();
    for pair in raw.dependencies {
        if pair.len() != 2 {
            return Err(LlmError::ParseError {
                raw: json.to_string(),
                reason: format!(
                    "dependency pair must have exactly 2 elements, got {}",
                    pair.len()
                ),
            });
        }
        let (before, after) = (pair[0], pair[1]);
        if before >= slice_count || after >= slice_count {
            return Err(LlmError::ParseError {
                raw: json.to_string(),
                reason: format!(
                    "dependency index out of bounds: ({before}, {after}) for {slice_count} slices"
                ),
            });
        }
        dependencies.push((before, after));
    }

    Ok(Plan {
        goal_text: if raw.goal_text.is_empty() {
            goal_text.to_string()
        } else {
            raw.goal_text
        },
        kind,
        complexity: Complexity {
            score: raw.complexity_score.clamp(1, 10),
            reasoning: raw.complexity_reasoning,
            estimated_hours: raw.estimated_hours,
        },
        slices,
        dependencies,
        acceptance_criteria: raw.acceptance_criteria,
        estimated_tokens: raw.estimated_tokens,
    })
}

// ============================================================================
// Classification
// ============================================================================

#[derive(Debug, Deserialize, Default)]
struct ClassificationJson {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    confidence: f32,
    #[serde(default)]
    reasoning: String,
    #[serde(default)]
    is_testable: bool,
    #[serde(default)]
    suggested_refinement: Option<String>,
}

/// Parse a JSON string into a [`GoalClassification`].
pub fn parse_classification_json(json: &str) -> Result<GoalClassification, LlmError> {
    let value = parse_json_value(json)?;
    let raw: ClassificationJson =
        serde_json::from_value(value).map_err(|e| LlmError::ParseError {
            raw: json.to_string(),
            reason: format!("classification JSON structure mismatch: {e}"),
        })?;

    Ok(GoalClassification {
        kind: parse_goal_kind(&raw.kind),
        confidence: raw.confidence.clamp(0.0, 1.0),
        reasoning: raw.reasoning,
        is_testable: raw.is_testable,
        suggested_refinement: raw.suggested_refinement,
    })
}

// ============================================================================
// Criteria
// ============================================================================

/// Parse a JSON string into a list of acceptance criteria.
pub fn parse_criteria_json(json: &str) -> Result<Vec<String>, LlmError> {
    let value = parse_json_value(json)?;

    // Accept either a top-level array or an object with a "criteria" field.
    let criteria: Vec<String> = if let Some(arr) = value.as_array() {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    } else if let Some(arr) = value.get("criteria").and_then(|v| v.as_array()) {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    } else {
        return Err(LlmError::ParseError {
            raw: json.to_string(),
            reason: "criteria JSON must be an array or an object with a 'criteria' field"
                .to_string(),
        });
    };

    Ok(criteria)
}

// ============================================================================
// Complexity
// ============================================================================

/// Parse a JSON string into a [`Complexity`].
pub fn parse_complexity_json(json: &str) -> Result<Complexity, LlmError> {
    let value = parse_json_value(json)?;

    let score = value
        .get("score")
        .and_then(|v| v.as_u64())
        .map(|n| n.clamp(1, 10) as u8)
        .unwrap_or(5);

    let reasoning = value
        .get("reasoning")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let estimated_hours = value
        .get("estimated_hours")
        .and_then(|v| v.as_f64())
        .map(|f| f as f32);

    Ok(Complexity {
        score,
        reasoning,
        estimated_hours,
    })
}

// ============================================================================
// Helpers
// ============================================================================

fn parse_goal_kind(raw: &str) -> GoalKind {
    match raw.to_lowercase().trim() {
        "greenfield" => GoalKind::Greenfield,
        "rewrite" => GoalKind::Rewrite,
        "repair" => GoalKind::Repair,
        "audit" => GoalKind::Audit,
        "migration" => GoalKind::Migration,
        _ => GoalKind::Vague,
    }
}

fn parse_difficulty(raw: &str) -> Difficulty {
    match raw.to_lowercase().trim() {
        "trivial" => Difficulty::Trivial,
        "easy" => Difficulty::Easy,
        "medium" => Difficulty::Medium,
        "hard" => Difficulty::Hard,
        "complex" => Difficulty::Complex,
        _ => Difficulty::Medium,
    }
}

#[cfg(test)]
mod tests;
