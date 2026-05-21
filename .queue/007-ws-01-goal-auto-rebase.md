---
id: 007
title: WS-01 — auto-rebase on merge-tree conflict before recording evidence
status: wip
branch: ws/goal-auto-rebase-on-merge-tree
worktree: (worker creates)
blocked_by: [001]
merge_after: [001]
size: medium
batch: audit-wave-1
pr: null
notes: Uses F-01 metadata fields (conflict_evidence_path, conflict_blocking_reason). Highest-value audit task — fills the gap where merge-tree detects conflicts but no auto-recovery follows.
---

# Prompt

Ты работаешь в репозитории /Users/ekhodzitsky/Documents/personal/oh-my-kimi.

ЦЕЛЬ: Сейчас при детекции merge-conflict через `git merge-tree` система записывает evidence, но НЕ пытается выполнить auto-rebase. Логика auto-rebase существует в delivery/slice_pr/rebase.rs (функция `ensure_slice_branch_merge_clean`), но используется только в slice PR delivery, не в worktree integration path. Нужно подключить её.

ВЕТКА: ws/goal-auto-rebase-on-merge-tree

СТРОГИЕ ПРАВИЛА:
1. Foundation-PR #109 уже смержен — пользуйся новыми полями metadata:
     - conflict_evidence_path: Option<PathBuf>
     - conflict_blocking_reason: Option<String>
   (slice_lease_id не трогай, это для WS-03 отдельно.)
2. ТРОГАЕШЬ только:
   - src/runtime/goal/worktree/conflict.rs
   - src/runtime/goal/delivery/slice_pr/rebase.rs (только если выносишь общую функцию; постарайся не менять её сигнатуру)
   - src/runtime/goal/control/until_ready/git.rs (только если там есть дублирующая логика, которую заменяешь вызовом единого хелпера)
   - НОВЫЙ файл: src/runtime/goal/git_ops/mod.rs + src/runtime/goal/git_ops/auto_rebase.rs (либо положи общий хелпер прямо в worktree/, если git_ops/ ещё нет)
3. НЕ ТРОГАЙ: review/, planner/, db/, любые модули вне списка выше.
4. НЕ МЕНЯЙ публичные сигнатуры существующих pub fn — расширяй через новые обёртки.

РАЗВЕДКА (обязательно до правок):
1. src/runtime/goal/worktree/conflict.rs целиком — функция `detect_goal_merge_conflicts` (или близкое имя). Что она делает после обнаружения конфликта?
2. src/runtime/goal/delivery/slice_pr/rebase.rs — функция `ensure_slice_branch_merge_clean`. Какие у неё аргументы? Что делает, что возвращает?
3. src/runtime/goal/control/until_ready/git.rs — функция `merge_branch_into_integrator` (около строки 56-94). Есть ли там emergency auto-rebase fallback? Если да — это template для нового кода.
4. src/git/repo.rs — какие методы GitRepo доступны для rebase/merge?

ЗАДАЧА:
В detect_goal_merge_conflicts (или эквивалент): после успешной детекции конфликта через merge-tree, ДО записи evidence, попытаться auto-rebase затронутой ветки на target (integrator). Логика:

    if merge_tree_reports_conflict {
        match attempt_auto_rebase(&slice_branch, &target_branch) {
            Ok(RebaseOutcome::Clean) => {
                // повторная проверка merge-tree
                // если clean — снимаем conflict, продолжаем delivery
            }
            Ok(RebaseOutcome::ConflictUnresolvable) | Err(_) => {
                // как сейчас: пишем evidence, ставим
                // conflict_evidence_path и conflict_blocking_reason
                // в delivery metadata
            }
        }
    }

`attempt_auto_rebase` — это новая функция (или вынесенная из slice_pr/rebase.rs обёртка). Она НЕ должна паниковать, всё через Result.

Evidence (когда rebase не помог):
- conflict_evidence_path — путь к артефакту с git-merge-tree output
- conflict_blocking_reason — human-readable: например, "auto_rebase_failed: <error>" или "merge_tree_conflict_unresolvable"

Тест: tests/goal_auto_rebase_test.rs (новый):
- Сценарий 1: два branch'а, изменения в разных файлах → merge-tree clean → auto_rebase не вызывается, evidence пуста.
- Сценарий 2: изменения в одном файле, разные строки (rebase решает) → rebase clean, повтор merge-tree — clean → evidence пуста.
- Сценарий 3: реальный merge conflict (одна и та же строка) → rebase падает или оставляет conflict markers → evidence записана, conflict_blocking_reason заполнен.

Используй TempDir + GitRepo для setup'а тестового git-репо (см. src/git/tests.rs).

SUCCESS CRITERIA:
- cargo build --all-targets — зелено
- cargo test — все существующие зелёные + три новых теста зелёные
- cargo clippy --all-targets -- -D warnings — зелено
- cargo fmt --check — зелено
- git diff — только файлы из scope выше

ЕСЛИ ЗАСТРЯЛ:
Если ensure_slice_branch_merge_clean имеет сложную сигнатуру (например, требует полный GoalRuntime контекст), которую нельзя переиспользовать в worktree/conflict.rs без больших рефакторингов — ОСТАНОВИСЬ и опиши проблему. Не пиши параллельную копию логики.

COMMIT: один атомарный `feat(goal): auto-rebase on merge-tree conflict before recording evidence`
PR title: то же.
