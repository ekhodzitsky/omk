# Killer Demo Fixture (CI-safe)

Этот демо-сценарий не требует реального Kimi CLI.
Он использует scripted fixture из тестов и изолированный `HOME`/`XDG_*`, поэтому не мутирует ваш реальный Kimi/OMK state.

## Что проверяется

- `success` outcome
- `failed verification` outcome (через `verification` gate fail)
- `stalled worker` outcome
- стабильный `demo-output.txt` (детерминированный формат для proof/demo)

## Связь с North Star demo

- `scripts/north_star_demo.sh` в режиме `MOCK_KIMI=1` использует тот же mock-first контракт:
  изолированный `HOME/XDG_*`, отсутствие реальных Kimi-зависимостей, сохранение реального Rust toolchain через
  `CARGO_HOME`/`RUSTUP_HOME` и явная обработка proof-статуса.
- В mock-режиме North Star demo сначала доказывает, что fixture падает, затем чинит его детерминированно,
  чтобы локальный proof/HUD сценарий завершался `ready` без сетевых вызовов.
- `examples/killer-demo` — это CI fixture-эталон для стабильности demo-сигнала (`demo-output.txt`),
  а North Star demo — операторский smoke surface с тем же ожидаемым поведением.

## Запуск

```bash
examples/killer-demo/run.sh
```

Скрипт:

1. создаёт временный каталог;
2. изолирует `HOME`, `XDG_STATE_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`;
3. запускает точечный тест:

```bash
cargo test --test mock_kimi_test test_team_demo_fixture_scripted_outcomes_are_stable -- --nocapture
```

4. удаляет временные файлы в `trap` cleanup.

## Артефакты fixture

Во время теста fixture пишет:

- `proof.json`
- `proof.md`
- `demo-output.txt` (стабильный строковый snapshot)

`demo-output.txt` предназначен для проверок в CI и коротких demo-report сценариев.
