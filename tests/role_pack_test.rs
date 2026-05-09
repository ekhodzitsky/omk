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
