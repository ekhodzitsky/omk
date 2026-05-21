---
id: 006
title: WS-29 — re-enable full Ubuntu CI matrix
status: wip
branch: ws/ci-restore-ubuntu-build
worktree: (worker creates)
blocked_by: []
merge_after: []
size: small
batch: audit-wave-1
pr: null
notes: Coordination exception — task DOES touch .github/workflows/ci.yml because that's the whole point. CI may red on first push if Linux-specific issues exist; that's expected and a separate follow-up.
---

# Prompt

Ты работаешь в репозитории /Users/ekhodzitsky/Documents/personal/oh-my-kimi.

ЦЕЛЬ: В .github/workflows/ci.yml ubuntu-latest matrix-entry помечен как `full: false` (commit 94ee741 "ci: temporarily disable Ubuntu build — macOS-first runtime"). Это значит на Ubuntu тесты не гоняются. Нужно включить полный прогон.

ВЕТКА: ws/ci-restore-ubuntu-build

СТРОГИЕ ПРАВИЛА:
1. Трогаешь ТОЛЬКО .github/workflows/ci.yml. Возможно also .github/workflows/coverage.yml если там тот же паттерн.
2. НЕ ТРОГАЙ release.yml, stale.yml, прочую инфру.
3. НЕ меняй version-pins на actions (SHA-pinning сохраняется).
4. НЕ повышай / не понижай MSRV.

РАЗВЕДКА:
- cat .github/workflows/ci.yml целиком
- найди matrix секцию с os/full
- посмотри, какие шаги пропускаются при full: false (через if: matrix.full)

ЗАДАЧА:
- Установи `full: true` для ubuntu-latest.
- В коде проверь, нет ли cfg(target_os = "macos") внутри тестов, которые сломают ubuntu. Если есть — перечисли их в PR body как known-incompatible, но НЕ ФИКСИ здесь (отдельный PR).

Если CI на ubuntu начнёт падать из-за отсутствующих системных зависимостей (libssl, sqlite3-dev), добавь шаг install:
    - name: Install system deps (ubuntu)
      if: matrix.os == 'ubuntu-latest'
      run: sudo apt-get update && sudo apt-get install -y <packages>
Но НЕ ИЗМЕНЯЙ structure jobs/steps кроме этого.

SUCCESS CRITERIA:
- `actionlint .github/workflows/ci.yml` зелено (если установлен; иначе пропусти, в PR body укажи это)
- diff минимальный — только `full: true` + опционально один install-step
- PR body: "после merge: первый запуск CI на ubuntu может упасть на linux-specific issues — следить, чинить отдельным PR"

COMMIT: `ci(ubuntu): re-enable full test matrix on ubuntu-latest`
