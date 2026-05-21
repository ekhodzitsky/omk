---
id: 004
title: WS-04 — concurrent supervisor claim race test
status: wip
branch: ws/goal-concurrent-controller-race-test
worktree: (worker creates)
blocked_by: []
merge_after: []
size: small
batch: audit-wave-1
pr: null
notes: Pure-test PR, no production logic change. Reveals race bug if SQLite isolation allows last-write-wins.
---

# Prompt

Ты работаешь в репозитории /Users/ekhodzitsky/Documents/personal/oh-my-kimi (Rust, axum, cargo).

Цель: написать поведенческий тест, который проверяет реакцию системы на гонку при попытке двух процессов одновременно claim_goal один и тот же goal_id. Аудит проекта показал: механика существует (supervisor.rs использует SQLite UPSERT + PID + heartbeat), НО ни одного теста под contention нет. Если SQLite-изоляция (DEFERRED) допускает last-write-wins, у нас потенциальная rotten state — это лучше вскрыть сейчас, а не после того, как поверх будет построен WS-03 (ownership leases).

СТРОГИЕ ПРАВИЛА:
1. Ветка: `ws/goal-concurrent-controller-race-test`.
2. НЕ ТРОГАЙ: ROADMAP.md, TODO.md, CHANGELOG.md, любые README.md, Cargo.toml, .gitignore, любые файлы вне списка из задачи ниже.
3. НЕ изменяй production-логику в src/runtime/goal/supervisor.rs. Только тесты. Если для теста нужен паблик хелпер — добавляй его ТОЛЬКО под `#[cfg(test)]` либо в test-utils модуль.
4. Тест должен быть ДЕТЕРМИНИРОВАННЫМ. Никаких `sleep(100)` для синхронизации потоков. Используй `tokio::sync::Barrier` или `std::sync::Barrier`.
5. cargo test, cargo clippy --all-targets -- -D warnings, cargo fmt — все три зелёные перед коммитом. Один атомарный коммит.
6. Если тест обнаруживает реальный race-баг — НЕ ФИКСИ. Документируй findings в PR body и пометь тест #[ignore = "race condition: tracking in WS-04 follow-up"].

РАЗВЕДКА:
1. src/runtime/goal/supervisor.rs целиком — `claim_goal()`, `release_goal()`, `heartbeat_*`. Точная сигнатура? Что возвращают при успехе/конфликте?
2. src/runtime/db/handle.rs — как открывается соединение, PRAGMAs (WAL, busy_timeout?), пул vs одиночное.
3. src/runtime/db/tests/concurrent.rs — там УЖЕ есть test_concurrent_writes / test_concurrent_reads_during_write. Возьми их паттерн.
4. src/runtime/goal/supervisor.rs::#[cfg(test)] — controller_pid_roundtrip, heartbeat_updates_timestamp, list_orphaned_detects_dead_pid. Используй тот же setup.
5. src/runtime/goal/types.rs — как создаётся валидный GoalId / GoalState.

ЗАДАЧА: один новый файл tests/goal_supervisor_concurrent_test.rs с тремя тестами:

TEST 1: `two_concurrent_claims_on_same_goal_have_exactly_one_winner`
- Создать temp-каталог + SQLite-БД.
- Создать один валидный goal.
- Поднять Barrier(2).
- Спавнить два tokio::task, каждый: ждёт барьер, потом вызывает supervisor::claim_goal с РАЗНЫМИ PID.
- Дождаться обеих задач через tokio::join!.
- Утверждения: РОВНО один claim вернул Ok(success); второй — Ok(false)/Err(AlreadyClaimed). controller_pid в БД == PID победителя.

TEST 2: `claim_then_release_then_reclaim_by_second_process_succeeds`
- Линейный baseline: claim(A) → release → claim(B) → controller_pid == B.

TEST 3: `concurrent_claim_during_stale_heartbeat_lets_new_owner_win`
- claim(A) → эмуляция stale heartbeat (прямой UPDATE) → конкурентный claim(B) и claim(C) через Barrier.
- Утверждение: ровно один из B/C победил; A не владелец.

ЕСЛИ ИНФРАСТРУКТУРА НЕ ПОЗВОЛЯЕТ:
Если supervisor требует целиком запущенный goal runtime и собрать минимальный test setup нереально без рефакторинга production API — ОСТАНОВИСЬ, напиши отчёт в PR body. Не пиши flaky-тест с sleep'ами, не моки половины supervisor.

SUCCESS CRITERIA:
- cargo build --all-targets зелено
- cargo test --test goal_supervisor_concurrent_test зелено
- cargo test целиком — ничего смежного не сломано
- cargo clippy --all-targets -- -D warnings зелено
- cargo fmt --check зелено
- git status: РОВНО один новый файл tests/goal_supervisor_concurrent_test.rs (или + #[cfg(test)] helper в supervisor.rs)
- Один коммит: `test(goal): concurrent supervisor claim race coverage`
