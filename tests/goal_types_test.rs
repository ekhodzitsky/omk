use omk::runtime::goal::{GoalBudget, GoalId, GoalKind};

#[test]
fn goal_id_generation_and_parsing_keep_ids_path_safe() {
    let generated = GoalId::generate();
    let parsed = GoalId::parse(generated.as_str()).expect("generated id should parse");

    assert_eq!(parsed.as_str(), generated.as_str());
    assert!(generated.as_str().starts_with("goal-"));
    assert!(!generated.as_str().contains('/'));
    assert!(!generated.as_str().contains(".."));
    assert!(GoalId::parse("../goal-escape").is_err());
    assert!(GoalId::parse("goal\nbad").is_err());
}

#[test]
fn goal_budget_preserves_optional_limits_and_rejects_invalid_values() {
    let budget = GoalBudget::new(Some("8h".to_string()), Some(1_000_000), Some(25.0), Some(3))
        .expect("valid budget");

    assert_eq!(budget.time.as_deref(), Some("8h"));
    assert_eq!(budget.tokens, Some(1_000_000));
    assert_eq!(budget.usd, Some(25.0));
    assert_eq!(budget.max_agents, Some(3));
    assert!(GoalBudget::new(None, Some(0), None, None).is_err());
    assert!(GoalBudget::new(None, None, Some(f64::NAN), None).is_err());
    assert!(GoalBudget::new(None, None, None, Some(0)).is_err());
}

#[test]
fn goal_kind_has_stable_machine_strings() {
    assert_eq!(GoalKind::Greenfield.as_str(), "greenfield");
    assert_eq!(GoalKind::Rewrite.as_str(), "rewrite");
    assert_eq!(GoalKind::Mixed.as_str(), "mixed");
}
