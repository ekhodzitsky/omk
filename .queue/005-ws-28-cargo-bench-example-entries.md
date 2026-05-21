---
id: 005
title: WS-28 — Cargo.toml [[bench]] and [[example]] entries
status: wip
branch: ws/cargo-toml-bench-example-entries
worktree: (worker creates)
blocked_by: []
merge_after: []
size: small
batch: audit-wave-1
pr: null
notes: 5-min PR. Unblocks `cargo bench` for entire project. EXCEPTION to coordination rule: this task DOES touch Cargo.toml (it's the whole point).
---

# Prompt

Ты работаешь в репозитории /Users/ekhodzitsky/Documents/personal/oh-my-kimi (Rust, cargo).

ЦЕЛЬ: сейчас `cargo bench` и `cargo run --example killer-demo` не работают, потому что в Cargo.toml нет соответствующих [[bench]] и [[example]] секций. Файлы существуют (benches/*.rs, examples/killer-demo/), но cargo о них не знает.

ВЕТКА: ws/cargo-toml-bench-example-entries

СТРОГИЕ ПРАВИЛА:
1. Трогаешь ТОЛЬКО Cargo.toml. Больше ничего.
2. НЕ добавляй новые dev-dependencies — criterion уже должен быть подключён, проверь это.
3. НЕ переименовывай существующие [package]/[dependencies] секции.

РАЗВЕДКА:
- ls benches/ → выпиши все *.rs файлы
- ls examples/ → выпиши все подкаталоги
- grep -n "criterion" Cargo.toml → убедись, что criterion есть в [dev-dependencies]
  Если нет — ОСТАНОВИСЬ.

ЗАДАЧА:
Для каждого файла benches/*.rs добавь в Cargo.toml:
    [[bench]]
    name = "<имя_без_расширения>"
    harness = false

Для examples/killer-demo (если это бинарный пример с main.rs):
    [[example]]
    name = "killer-demo"
    path = "examples/killer-demo/<main_file>.rs"
ПРОВЕРЬ заранее реальный путь к точке входа. Если killer-demo не является rust-крейтом (например, shell-скрипты в Makefile) — НЕ добавляй [[example]], пометь в PR body как not-a-cargo-example.

SUCCESS CRITERIA:
- cargo build --benches — зелено
- cargo bench --no-run — зелено (компилируется, не запускаем)
- cargo build --examples — зелено (если добавлен [[example]])
- cargo test — не сломалось
- cargo clippy --all-targets -- -D warnings — зелено

COMMIT: один коммит `build(cargo): register benches and examples`
PR title: `build(cargo): register benches and examples`
