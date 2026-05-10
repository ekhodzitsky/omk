use omk::kimi_native::role_packs::RolePack;

#[test]
fn test_role_pack_all_has_5_roles() {
    let roles = RolePack::all();
    assert_eq!(roles.len(), 5);
}

#[test]
fn test_role_pack_find_architect() {
    let pack = RolePack::find("architect");
    assert!(pack.is_some());
    let pack = pack.unwrap();
    assert_eq!(pack.id, "architect");
    assert_eq!(pack.name, "Architect");
    assert_eq!(pack.suggested_worker_count, 1);
}

#[test]
fn test_role_pack_find_unknown_returns_none() {
    let pack = RolePack::find("nonexistent");
    assert!(pack.is_none());
}

#[test]
fn test_executor_has_test_tool() {
    let pack = RolePack::find("executor").unwrap();
    assert!(pack.tools.contains(&"test".to_string()));
}

#[test]
fn test_all_roles_have_prompt_guards() {
    let roles = RolePack::all();
    for role in roles {
        assert!(
            role.system_prompt.contains("Instruction Hierarchy"),
            "{} is missing instruction hierarchy section",
            role.id
        );
        assert!(
            role.system_prompt.contains("AGENTS.md"),
            "{} must reference AGENTS.md hierarchy",
            role.id
        );
        assert!(
            role.system_prompt.contains("Anti-Slop"),
            "{} is missing anti-slop guardrails",
            role.id
        );
        assert!(
            role.system_prompt.contains("Review Discipline"),
            "{} is missing review discipline section",
            role.id
        );
    }
}

#[test]
fn test_role_pack_default_skills_are_repo_local() {
    let architect = RolePack::find("architect").unwrap();
    assert_eq!(architect.default_skills, vec!["architect".to_string()]);

    let executor = RolePack::find("executor").unwrap();
    assert_eq!(
        executor.default_skills,
        vec!["backend".to_string(), "qa".to_string()]
    );

    let verifier = RolePack::find("verifier").unwrap();
    assert_eq!(verifier.default_skills, vec!["qa".to_string()]);

    let integrator = RolePack::find("integrator").unwrap();
    assert_eq!(integrator.default_skills, vec!["devops".to_string()]);
}
