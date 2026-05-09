#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-.}"
cd "$ROOT"

section() {
  printf '\n## %s\n' "$1"
}

list_rust_files() {
  if command -v rg >/dev/null 2>&1; then
    rg --files -g '*.rs' | sort
  else
    find . -path './target' -prune -o -name '*.rs' -print | sed 's#^\./##' | sort
  fi
}

section "Repository"
if git rev-parse --show-toplevel >/dev/null 2>&1; then
  printf 'root: %s\n' "$(git rev-parse --show-toplevel)"
  printf 'head: %s\n' "$(git rev-parse --short HEAD 2>/dev/null || true)"
  printf 'branch: %s\n' "$(git branch --show-current 2>/dev/null || true)"
else
  printf 'root: %s\n' "$(pwd)"
fi

section "Working Tree"
if git rev-parse --show-toplevel >/dev/null 2>&1; then
  git status --short | sed -n '1,80p'
else
  printf 'not a git repository\n'
fi

section "Rust Lines By Area"
for dir in src src/cli src/runtime src/runtime/scheduler src/wire src/kimi_native src/vis src/skills src/mcp tests benches; do
  if [ -d "$dir" ]; then
    count=$(find "$dir" -type f -name '*.rs' -print | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')
    printf '%7s  %s\n' "${count:-0}" "$dir"
  fi
done

section "Largest Rust Files"
rust_files="$(list_rust_files)"
if [ -n "$rust_files" ]; then
  printf '%s\n' "$rust_files" | xargs wc -l | sort -nr | sed -n '1,30p'
else
  printf 'no Rust files found\n'
fi

section "Module Declarations"
if command -v rg >/dev/null 2>&1; then
  { rg -n '^(pub )?mod [a-zA-Z0-9_]+;' src || true; } | sed -n '1,120p'
else
  { grep -R -n -E '^(pub )?mod [a-zA-Z0-9_]+;' src || true; } | sed -n '1,120p'
fi

section "CLI Touchpoints"
if command -v rg >/dev/null 2>&1; then
  { rg -n 'Subcommand|Parser|Args|enum .*Command|pub async fn|pub fn' src/main.rs src/cli || true; } | sed -n '1,160p'
else
  { grep -R -n -E 'Subcommand|Parser|Args|enum .*Command|pub async fn|pub fn' src/main.rs src/cli || true; } | sed -n '1,160p'
fi

section "Tests"
find tests benches -type f -name '*.rs' -print 2>/dev/null | sort

section "Agent And Documentation Entrypoints"
find .kimi docs src -maxdepth 3 \( -name 'README.md' -o -name 'PROJECT_MAP.md' -o -name 'SKILL.md' \) -print 2>/dev/null | sort

section "Recommended First Reads"
cat <<'EOF'
docs/PROJECT_MAP.md
src/cli/README.md
src/runtime/README.md
src/wire/README.md
src/kimi_native/README.md
.kimi/skills/omk-navigation/SKILL.md
EOF
