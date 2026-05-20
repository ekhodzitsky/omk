use super::types::{Plan, RepoContext};

/// Combine a system message and user prompt into a single prompt string.
///
/// This is the canonical format used by both [`MockLlmClient`](crate::llm::MockLlmClient)
/// and [`WireLlmClient`](crate::llm::WireLlmClient).
pub fn system_prompt(system: &str, prompt: &str) -> String {
    format!("SYSTEM:\n{system}\n\nUSER:\n{prompt}")
}

const CLASSIFICATION_VERSION: &str = "v1";
const DECOMPOSITION_VERSION: &str = "v1";
const CRITERIA_VERSION: &str = "v1";
const COMPLEXITY_VERSION: &str = "v1";

/// Build a prompt that asks the LLM to classify a goal.
pub fn classification_prompt(goal_text: &str) -> String {
    format!(
        r#"You are a software engineering planner.

Analyze the following goal and classify it into exactly one category:
- greenfield: building something new
- rewrite: significantly changing existing code
- repair: fixing a bug or error
- audit: reviewing code for issues without changing it
- migration: moving code between frameworks, languages, or architectures
- vague: unclear or underspecified

Respond with a JSON object containing:
- kind: one of the categories above
- confidence: float between 0.0 and 1.0
- reasoning: brief explanation
- is_testable: boolean indicating whether the goal is testable
- suggested_refinement: string or null, suggesting how to clarify if vague

Goal: {goal_text}

prompt_version: {CLASSIFICATION_VERSION}"#
    )
}

/// Build a prompt that asks the LLM to decompose a goal into slices.
pub fn decomposition_prompt(goal_text: &str, context: &RepoContext) -> String {
    let lang = context.primary_language.as_deref().unwrap_or("unknown");
    let files = context.top_level_files.join(", ");

    format!(
        r#"You are a software engineering planner.

Decompose the following goal into a sequence of implementation slices.
Each slice should be a coherent, reviewable unit of work.

Repository context:
- Primary language: {lang}
- File count: {file_count}
- Top-level files: {files}
- Has tests: {has_tests}
- Has CI: {has_ci}

Respond with a JSON object containing:
- goal_text: the original goal
- kind: one of greenfield, rewrite, repair, audit, migration, vague
- complexity_score: integer 1-10
- complexity_reasoning: brief explanation
- estimated_hours: float or null
- slices: array of objects with id, description, write_set (array of file paths), estimated_difficulty (trivial/easy/medium/hard/complex)
- dependencies: array of [before, after] index pairs into slices
- acceptance_criteria: array of strings
- estimated_tokens: approximate token count for this plan

Goal: {goal_text}

prompt_version: {DECOMPOSITION_VERSION}"#,
        lang = lang,
        file_count = context.file_count,
        files = files,
        has_tests = context.has_tests,
        has_ci = context.has_ci,
        goal_text = goal_text,
        DECOMPOSITION_VERSION = DECOMPOSITION_VERSION,
    )
}

/// Build a prompt that asks the LLM to generate acceptance criteria.
pub fn criteria_prompt(goal_text: &str, plan: &Plan) -> String {
    let slices_summary = plan
        .slices
        .iter()
        .map(|s| format!("- {}: {}", s.id, s.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are a software engineering planner.

Generate concrete, verifiable acceptance criteria for the following goal and plan.

Goal: {goal_text}

Plan slices:
{slices_summary}

Respond with a JSON array of criterion strings, or an object with a "criteria" field containing the array.

prompt_version: {CRITERIA_VERSION}"#
    )
}

/// Build a prompt that asks the LLM to estimate complexity.
pub fn complexity_prompt(goal_text: &str, plan: &Plan) -> String {
    format!(
        r#"You are a software engineering planner.

Estimate the complexity of the following goal and plan.

Goal: {goal_text}

Number of slices: {slice_count}
Dependencies: {dep_count}

Respond with a JSON object containing:
- score: integer 1-10
- reasoning: brief explanation
- estimated_hours: float or null

prompt_version: {COMPLEXITY_VERSION}"#,
        slice_count = plan.slices.len(),
        dep_count = plan.dependencies.len(),
        COMPLEXITY_VERSION = COMPLEXITY_VERSION,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_version_present() {
        let cls = classification_prompt("Test goal");
        assert!(cls.contains("prompt_version: v1"));

        let dec = decomposition_prompt(
            "Test goal",
            &RepoContext {
                primary_language: Some("Rust".to_string()),
                file_count: 10,
                top_level_files: vec!["src".to_string()],
                has_tests: true,
                has_ci: false,
            },
        );
        assert!(dec.contains("prompt_version: v1"));

        let crit = criteria_prompt(
            "Test goal",
            &Plan {
                goal_text: "Test".to_string(),
                kind: super::super::types::GoalKind::Greenfield,
                complexity: super::super::types::Complexity {
                    score: 5,
                    reasoning: "Medium".to_string(),
                    estimated_hours: None,
                },
                slices: vec![],
                dependencies: vec![],
                acceptance_criteria: vec![],
                estimated_tokens: 0,
            },
        );
        assert!(crit.contains("prompt_version: v1"));

        let comp = complexity_prompt(
            "Test goal",
            &Plan {
                goal_text: "Test".to_string(),
                kind: super::super::types::GoalKind::Greenfield,
                complexity: super::super::types::Complexity {
                    score: 5,
                    reasoning: "Medium".to_string(),
                    estimated_hours: None,
                },
                slices: vec![],
                dependencies: vec![],
                acceptance_criteria: vec![],
                estimated_tokens: 0,
            },
        );
        assert!(comp.contains("prompt_version: v1"));
    }
}
