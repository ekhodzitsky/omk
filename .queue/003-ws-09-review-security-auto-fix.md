---
id: 003
title: WS-09 — auto-spawn security cleanup tasks from verifier findings
status: wip
branch: ws/review-security-auto-fix
worktree: .worktrees/agent-security-auto-fix
blocked_by: [001]
merge_after: [001]
size: small
batch: audit-wave-1
pr: null
notes: Originally dispatched 2026-05-21 in parallel batch after foundation merge.
---

# Prompt

Ты работаешь в репозитории /Users/ekhodzitsky/Documents/personal/oh-my-kimi.

ЦЕЛЬ: Anti-slop review автоматически spawn'ит refactor-задачу при наличии находок (см. commit b2dba1f). Для security findings такого нет — они просто блокируют delivery. Нужно добавить аналогичный auto-spawn для security findings, чтобы агент мог попытаться починить secret-leak до того, как human вмешается.

ВЕТКА: ws/review-security-auto-fix

СТРОГИЕ ПРАВИЛА:
1. ТРОГАЕШЬ:
   - src/runtime/goal/lifecycle/cleanup.rs (главное)
   - src/runtime/goal/verifier/security.rs (если нужно расширить возвращаемую структуру SecurityFinding — добавляй поля, не ломая существующие)
2. НЕ ТРОГАЙ: review/, planner/, dispatch/, db/.
3. НЕ ПЕРЕИЗОБРЕТАЙ: следуй паттерну anti-slop spawn'а (spawn_refactor_task_from_slop_findings в lifecycle/cleanup.rs).

РАЗВЕДКА:
1. src/runtime/goal/lifecycle/cleanup.rs — функция spawn_refactor_task_from_slop_findings. Изучи: как формируется prompt для нового task'а, как он попадает в task graph, что возвращается.
2. src/runtime/goal/verifier/security.rs — что возвращает security verifier (SecurityFinding структура?). Какие kind'ы находок есть (private key, secret assignment, symlink escape, oversized file).
3. process_slice_delivery_and_review() — где вызывается anti-slop spawn. Параллельный hook для security должен быть РЯДОМ, не дублироваться в другом месте.

ЗАДАЧА:
В lifecycle/cleanup.rs:

    pub(crate) fn spawn_security_cleanup_task_from_findings(
        ctx: &/* тот же тип контекста, что у anti-slop spawn */,
        findings: &[SecurityFinding],
    ) -> anyhow::Result<Option<SpawnedTaskId>> {
        if findings.is_empty() {
            return Ok(None);
        }

        // 1. Сформировать prompt: "Remove the following security findings from the slice: ..."
        //    Для каждой находки описать: path, kind (private_key / secret_assignment /
        //    symlink_escape / oversized), line if available.
        //    ВАЖНО: не включать в prompt содержимое самого секрета (значение private_key или
        //    token value). Только указание "remove the secret on path X line Y".
        //    Иначе мы протащим секрет в task description, который запишется в events.jsonl.
        //
        // 2. Создать task'у того же kind/severity, что и refactor от anti-slop, но с
        //    owner_role = "security-cleanup" (или что подходит по существующей шкале).
        //
        // 3. Вернуть Some(spawned_id) или None если все findings quarantine-only.
    }

В точке, где сейчас вызывается spawn_refactor_task_from_slop_findings, добавить РЯДОМ вызов spawn_security_cleanup_task_from_findings (если у данной фазы есть security findings).

КРИТИЧНО — REDACTION:
- Если SecurityFinding содержит field `evidence_snippet` или подобный с фактическим текстом секрета — ОБЯЗАТЕЛЬНО redact его перед попаданием в prompt. Поиск: src/cost/ или src/runtime/ может иметь redact_secret() helper. Если нет — реализуй inline через простую замену на "<REDACTED>".

Тест в cleanup.rs#[cfg(test)] mod tests:
- spawn_security_cleanup_returns_none_on_empty_findings
- spawn_security_cleanup_creates_task_for_private_key_finding
- spawn_security_cleanup_does_not_leak_secret_value_into_task_prompt (assert, что в task prompt НЕ содержится сырого secret value)
- spawn_security_cleanup_skips_quarantine_only_findings (oversized_file should not trigger auto-fix; needs human)

SUCCESS CRITERIA:
- cargo build --all-targets — зелено
- cargo test — зелено
- cargo clippy --all-targets -- -D warnings — зелено
- cargo fmt --check — зелено
- diff только в lifecycle/cleanup.rs (+ возможно security.rs)
- PR body содержит явное упоминание redaction-теста как guard против secret leakage

ЕСЛИ ЗАСТРЯЛ:
- Если SecurityFinding не содержит field, который позволяет различить "auto-fixable" vs "quarantine-only" — НЕ ВЫДУМЫВАЙ. ОСТАНОВИСЬ и опиши.
- Если spawn_refactor_task_from_slop_findings глубоко завязан на AntiSlopFinding-specific полях — выдели общий helper, но это потенциально лезет в slop.rs, что вне scope. В этом случае — реализуй inline в новой функции, дубликат ОК на этом этапе.

COMMIT: `feat(review): auto-spawn security cleanup tasks from verifier findings`
