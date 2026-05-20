use super::*;

#[test]
fn test_parse_plan_json_valid() {
    let json = r#"{
        "goal_text": "Add auth",
        "kind": "greenfield",
        "complexity_score": 5,
        "complexity_reasoning": "Medium complexity",
        "estimated_hours": 4.0,
        "slices": [
            {
                "id": "slice-1",
                "description": "Implement login",
                "write_set": ["src/auth.rs"],
                "estimated_difficulty": "medium"
            },
            {
                "id": "slice-2",
                "description": "Add tests",
                "write_set": ["tests/auth.rs"],
                "estimated_difficulty": "easy"
            }
        ],
        "dependencies": [[0, 1]],
        "acceptance_criteria": ["User can log in"],
        "estimated_tokens": 1000
    }"#;

    let plan = parse_plan_json("Add auth", json).unwrap();
    assert_eq!(plan.goal_text, "Add auth");
    assert_eq!(plan.kind, GoalKind::Greenfield);
    assert_eq!(plan.complexity.score, 5);
    assert_eq!(plan.slices.len(), 2);
    assert_eq!(plan.slices[0].id, "slice-1");
    assert_eq!(plan.slices[1].id, "slice-2");
    assert_eq!(plan.slices[0].estimated_difficulty, Difficulty::Medium);
    assert_eq!(plan.dependencies, vec![(0, 1)]);
    assert_eq!(plan.acceptance_criteria, vec!["User can log in"]);
}

#[test]
fn test_parse_plan_json_markdown_wrapped() {
    let json = r#"```json
    {
        "kind": "rewrite",
        "slices": [],
        "dependencies": []
    }
    ```"#;

    let plan = parse_plan_json("Refactor", json).unwrap();
    assert_eq!(plan.kind, GoalKind::Rewrite);
    assert!(plan.slices.is_empty());
}

#[test]
fn test_parse_plan_json_markdown_any_lang() {
    let json = r#"```javascript
    {"kind": "audit", "slices": [], "dependencies": []}
    ```"#;

    let plan = parse_plan_json("Audit", json).unwrap();
    assert_eq!(plan.kind, GoalKind::Audit);
}

#[test]
fn test_parse_plan_json_missing_field() {
    let json = r#"{"kind": "repair"}"#;

    let plan = parse_plan_json("Fix bug", json).unwrap();
    assert_eq!(plan.kind, GoalKind::Repair);
    assert!(plan.slices.is_empty());
    assert!(plan.acceptance_criteria.is_empty());
    assert_eq!(plan.goal_text, "Fix bug");
}

#[test]
fn test_parse_plan_json_missing_id_uses_index() {
    let json = r#"{"slices": [{"description": "x"}]}"#;

    let plan = parse_plan_json("Test", json).unwrap();
    assert_eq!(plan.slices[0].id, "slice-0");
}

#[test]
fn test_parse_plan_json_invalid_type() {
    let json = r#"{"complexity_score": "not-a-number"}"#;

    let result = parse_plan_json("Test", json);
    assert!(matches!(result, Err(LlmError::ParseError { .. })));
}

#[test]
fn test_parse_plan_json_malformed_dependency() {
    let json = r#"{"dependencies": [[0]]}"#;

    let result = parse_plan_json("Test", json);
    assert!(matches!(result, Err(LlmError::ParseError { .. })));
}

#[test]
fn test_parse_classification_confidence() {
    let json = r#"{
        "kind": "audit",
        "confidence": 0.85,
        "reasoning": "Looks like audit",
        "is_testable": true
    }"#;

    let cls = parse_classification_json(json).unwrap();
    assert_eq!(cls.kind, GoalKind::Audit);
    assert!((cls.confidence - 0.85).abs() < f32::EPSILON);
    assert!(cls.is_testable);
}

#[test]
fn test_parse_criteria_json_array() {
    let json = r#"["User can login", "Session persists"]"#;
    let criteria = parse_criteria_json(json).unwrap();
    assert_eq!(criteria.len(), 2);
}

#[test]
fn test_parse_criteria_json_object() {
    let json = r#"{"criteria": ["Test passes", "No regressions"]}"#;
    let criteria = parse_criteria_json(json).unwrap();
    assert_eq!(criteria.len(), 2);
}

#[test]
fn test_parse_criteria_json_invalid() {
    let json = r#"{"foo": "bar"}"#;
    let result = parse_criteria_json(json);
    assert!(matches!(result, Err(LlmError::ParseError { .. })));
}

#[test]
fn test_parse_complexity_json() {
    let json = r#"{"score": 7, "reasoning": "Hard", "estimated_hours": 8.5}"#;
    let complexity = parse_complexity_json(json).unwrap();
    assert_eq!(complexity.score, 7);
    assert_eq!(complexity.reasoning, "Hard");
    assert!((complexity.estimated_hours.unwrap() - 8.5).abs() < f32::EPSILON);
}

#[test]
fn test_parse_complexity_json_defaults() {
    let json = r#"{}"#;
    let complexity = parse_complexity_json(json).unwrap();
    assert_eq!(complexity.score, 5);
    assert_eq!(complexity.reasoning, "");
    assert_eq!(complexity.estimated_hours, None);
}

#[test]
fn test_extract_json_inline_fence() {
    // No newline after opening fence — fallback path
    let input = "```json{\"a\": 1}```";
    let result = super::parse_json_value(input).unwrap();
    assert_eq!(result["a"], 1);
}

#[test]
fn test_parse_plan_json_goal_text_fallback() {
    let json = r#"{"kind": "greenfield"}"#;
    let plan = parse_plan_json("Original goal", json).unwrap();
    assert_eq!(plan.goal_text, "Original goal");
}

#[test]
fn test_parse_plan_json_dependency_out_of_bounds() {
    let json = r#"{"slices": [{"id": "s1"}], "dependencies": [[0, 5]]}"#;
    let result = parse_plan_json("Test", json);
    assert!(matches!(result, Err(LlmError::ParseError { .. })));
}
