---
id: 008
title: WS-05 — GitHub branch protection automation (opt-in)
status: wip
branch: ws/github-branch-protection-automation
worktree: (worker creates)
blocked_by: [001]
merge_after: [001]
size: medium
batch: audit-wave-1
pr: null
notes: Adds opt-in flag --enforce-protection. Default OFF to preserve existing workflows. Requires repo admin token at gh auth.
---

# Prompt

Ты работаешь в репозитории /Users/ekhodzitsky/Documents/personal/oh-my-kimi.

ЦЕЛЬ: Согласно ROADMAP Stage 5, master/main должны быть read-only baselines: всё через integrator PR, прямой push заблокирован branch protection rules. Сейчас этой автоматизации нет — рассчитываем на ручную настройку в GitHub UI. Нужно добавить опциональный шаг, который программно выставляет branch protection через `gh api` перед открытием integrator PR.

ВЕТКА: ws/github-branch-protection-automation

СТРОГИЕ ПРАВИЛА:
1. Новые файлы:
   - src/runtime/goal/delivery/github_api.rs (модуль для gh api вызовов не относящихся к PR — branch protection, etc.)
2. Можешь править:
   - src/runtime/goal/delivery/mod.rs (декларация `pub mod github_api;`)
   - src/runtime/goal/open_pr/mod.rs ИЛИ src/runtime/goal/control/until_ready/integrator.rs (точка вызова — перед созданием integrator PR)
   - src/cli/goal/mod.rs (новый флаг --enforce-protection, по умолчанию OFF)
3. НЕ ТРОГАЙ: review/, db/, worktree/, любые модули вне списка.
4. НЕ удаляй / не переименовывай существующие функции delivery.

РАЗВЕДКА:
1. src/runtime/goal/delivery/pr_client.rs — как сейчас делаются gh CLI вызовы. Используй ТОТ ЖЕ паттерн.
2. src/runtime/goal/control/until_ready/integrator.rs — найди точку, где создаётся integrator PR. Перед этим должен быть hook на enforce_branch_protection.
3. src/cli/goal/mod.rs — как сейчас передаются delivery-related флаги (--merge-policy). Сделай --enforce-protection в том же стиле.

ЗАДАЧА:

В src/runtime/goal/delivery/github_api.rs:

    pub struct BranchProtectionPolicy {
        pub required_status_checks: Vec<String>,
        pub required_review_count: u32,
        pub enforce_admins: bool,
        pub allow_force_pushes: bool,
        pub allow_deletions: bool,
    }

    impl Default for BranchProtectionPolicy { ... }
       // required_status_checks: vec![]
       // required_review_count: 1
       // enforce_admins: false
       // allow_force_pushes: false
       // allow_deletions: false

    pub async fn ensure_branch_protection(
        owner: &str,
        repo: &str,
        branch: &str,
        policy: &BranchProtectionPolicy,
    ) -> anyhow::Result<()>;
        // Вызов: gh api -X PUT repos/{owner}/{repo}/branches/{branch}/protection
        // Если protection уже выставлена и эквивалентна — no-op.
        // Если выставлена иначе — ПЕРЕЗАПИСЫВАЕТ (PUT, не PATCH).
        // Ошибки: 404 (нет repo / нет прав) → понятный error.
        //         403 (нет admin прав) → понятный error с подсказкой.

В точке вызова integrator PR:
- Если флаг --enforce-protection включён И target branch это main/master:
  вызвать ensure_branch_protection ПЕРЕД pr create. Required status checks = имена required gates.
- Если не включён: пропустить, лог "skipping branch protection setup (--enforce-protection not set)".

CLI:
- Добавить --enforce-protection (bool, default false) в команду run.
- Передать его сквозь до integrator step.

Тест:
- src/runtime/goal/delivery/github_api.rs#[cfg(test)] mod tests: юнит-тест на сериализацию BranchProtectionPolicy в json body согласно GitHub API схеме. НЕ ходи реально в GitHub — моки не нужны, проверяй ТОЛЬКО payload-строку.
- Если есть test-utils для CLI smoke (типа cli_goal_ux_test.rs), добавь тест что --enforce-protection корректно парсится и не ломает help.

SUCCESS CRITERIA:
- cargo build --all-targets — зелено
- cargo test — зелено
- cargo clippy --all-targets -- -D warnings — зелено
- cargo fmt --check — зелено
- `omk goal run --help` показывает --enforce-protection
- diff только в файлах списка scope
- PR body: explicit warning, что фича требует repo admin token у `gh auth`, иначе ensure_branch_protection вернёт ошибку

ЕСЛИ ЗАСТРЯЛ:
- Если pr_client использует не gh CLI а octocrab/octocrate — используй тот же крейт, не подключай новый.
- Если структура integrator step не позволяет вклинить шаг без большого refactor'а — ОСТАНОВИСЬ и опиши.

COMMIT: `feat(delivery): GitHub branch protection automation (opt-in)`
