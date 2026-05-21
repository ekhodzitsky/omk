use super::*;
use crate::llm::client::MockLlmClient;
use crate::llm::types::{GoalKind, Slice};

#[tokio::test]
async fn test_planner_classify_mock() {
    let classification_json = r#"{
        "kind": "greenfield",
        "confidence": 0.95,
        "reasoning": "Building new feature",
        "is_testable": true,
        "suggested_refinement": null
    }"#;

    let client = Arc::new(MockLlmClient::new(vec![classification_json.to_string()]));
    let planner = LlmPlanner::new(client, TokenBudget::new(1000));

    let result = planner.classify("Add user authentication").await.unwrap();
    assert_eq!(result.kind, GoalKind::Greenfield);
    assert!((result.confidence - 0.95).abs() < f32::EPSILON);
    assert!(result.is_testable);
}

#[tokio::test]
async fn test_planner_decompose_mock() {
    let plan_json = r#"{
        "kind": "rewrite",
        "complexity_score": 7,
        "complexity_reasoning": "Significant changes",
        "slices": [
            {
                "id": "slice-1",
                "description": "Refactor module A",
                "write_set": ["src/a.rs"],
                "estimated_difficulty": "hard"
            },
            {
                "id": "slice-2",
                "description": "Refactor module B",
                "write_set": ["src/b.rs"],
                "estimated_difficulty": "medium"
            }
        ],
        "dependencies": [[0, 1]],
        "acceptance_criteria": ["All tests pass"]
    }"#;

    let client = Arc::new(MockLlmClient::new(vec![plan_json.to_string()]));
    let planner = LlmPlanner::new(client, TokenBudget::new(1000));

    let result = planner
        .decompose(
            "Refactor core modules",
            &RepoContext {
                primary_language: Some("Rust".to_string()),
                file_count: 20,
                top_level_files: vec!["src".to_string()],
                has_tests: true,
                has_ci: true,
            },
        )
        .await
        .unwrap();

    assert_eq!(result.kind, GoalKind::Rewrite);
    assert_eq!(result.slices.len(), 2);
    assert_eq!(result.dependencies, vec![(0, 1)]);
    assert_eq!(
        result.slices[0].estimated_difficulty,
        super::super::types::Difficulty::Hard
    );
}

#[tokio::test]
async fn test_mock_planner_classify() {
    let planner = MockPlanner::new().with_classification(GoalClassification {
        kind: GoalKind::Audit,
        confidence: 0.8,
        reasoning: "Looks like audit".to_string(),
        is_testable: false,
        suggested_refinement: None,
    });

    let result = planner.classify("anything").await.unwrap();
    assert_eq!(result.kind, GoalKind::Audit);
}

#[tokio::test]
async fn test_mock_planner_decompose() {
    let planner = MockPlanner::new().with_plan(Plan {
        goal_text: "Test".to_string(),
        kind: GoalKind::Repair,
        complexity: Complexity {
            score: 3,
            reasoning: "Easy fix".to_string(),
            estimated_hours: Some(1.0),
        },
        slices: vec![Slice {
            id: "s1".to_string(),
            description: "Fix it".to_string(),
            write_set: vec!["src/lib.rs".to_string()],
            estimated_difficulty: super::super::types::Difficulty::Easy,
        }],
        dependencies: vec![],
        acceptance_criteria: vec!["Tests pass".to_string()],
        estimated_tokens: 100,
    });

    let result = planner
        .decompose(
            "Fix bug",
            &RepoContext {
                primary_language: None,
                file_count: 0,
                top_level_files: vec![],
                has_tests: false,
                has_ci: false,
            },
        )
        .await
        .unwrap();

    assert_eq!(result.kind, GoalKind::Repair);
    assert_eq!(result.slices.len(), 1);
}

#[tokio::test]
async fn test_planner_generate_criteria_mock() {
    let criteria_json = r#"["User can login", "Session persists"]"#;
    let client = Arc::new(MockLlmClient::new(vec![criteria_json.to_string()]));
    let planner = LlmPlanner::new(client, TokenBudget::new(1000));

    let plan = Plan {
        goal_text: "Add auth".to_string(),
        kind: GoalKind::Greenfield,
        complexity: Complexity {
            score: 5,
            reasoning: "Medium".to_string(),
            estimated_hours: None,
        },
        slices: vec![],
        dependencies: vec![],
        acceptance_criteria: vec![],
        estimated_tokens: 0,
    };

    let result = planner.generate_criteria("Add auth", &plan).await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], "User can login");
}

#[tokio::test]
async fn test_planner_estimate_complexity_mock() {
    let complexity_json = r#"{"score": 8, "reasoning": "Hard", "estimated_hours": 12.0}"#;
    let client = Arc::new(MockLlmClient::new(vec![complexity_json.to_string()]));
    let planner = LlmPlanner::new(client, TokenBudget::new(1000));

    let plan = Plan {
        goal_text: "Refactor core".to_string(),
        kind: GoalKind::Rewrite,
        complexity: Complexity {
            score: 5,
            reasoning: "Medium".to_string(),
            estimated_hours: None,
        },
        slices: vec![],
        dependencies: vec![],
        acceptance_criteria: vec![],
        estimated_tokens: 0,
    };

    let result = planner
        .estimate_complexity("Refactor core", &plan)
        .await
        .unwrap();
    assert_eq!(result.score, 8);
    assert_eq!(result.reasoning, "Hard");
    assert!((result.estimated_hours.unwrap() - 12.0).abs() < f32::EPSILON);
}

#[tokio::test]
async fn test_mock_planner_generate_criteria() {
    let planner = MockPlanner::new().with_criteria(vec!["Test passes".to_string()]);
    let plan = Plan {
        goal_text: "Test".to_string(),
        kind: GoalKind::Greenfield,
        complexity: Complexity {
            score: 1,
            reasoning: "Trivial".to_string(),
            estimated_hours: None,
        },
        slices: vec![],
        dependencies: vec![],
        acceptance_criteria: vec![],
        estimated_tokens: 0,
    };
    let result = planner.generate_criteria("Test", &plan).await.unwrap();
    assert_eq!(result, vec!["Test passes"]);
}

#[tokio::test]
async fn test_mock_planner_estimate_complexity() {
    let planner = MockPlanner::new().with_complexity(Complexity {
        score: 9,
        reasoning: "Complex".to_string(),
        estimated_hours: Some(20.0),
    });
    let plan = Plan {
        goal_text: "Test".to_string(),
        kind: GoalKind::Greenfield,
        complexity: Complexity {
            score: 1,
            reasoning: "Trivial".to_string(),
            estimated_hours: None,
        },
        slices: vec![],
        dependencies: vec![],
        acceptance_criteria: vec![],
        estimated_tokens: 0,
    };
    let result = planner.estimate_complexity("Test", &plan).await.unwrap();
    assert_eq!(result.score, 9);
}
