# `src/analysis/`

Tree-sitter based code analysis module.

## Purpose

Provides structured AST parsing and querying for multiple languages,
complementing regex-based analysis for scope detection, function extraction,
and call graph construction.

## Public API

- `parse_file(path, source)` — Parse source code into a `SyntaxTree`
- `find_function_definitions(tree)` — Extract function definitions
- `find_calls_to(tree, function_name)` — Find call sites for a function

## Supported Languages

- Rust (`.rs`)
- JavaScript/TypeScript (`.js`, `.jsx`, `.ts`, `.tsx`)
- Python (`.py`, `.pyi`)
- Go (`.go`)

## Status

Initial implementation. Covers basic function and call extraction.
