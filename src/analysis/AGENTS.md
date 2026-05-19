# analysis — Agent Guide

## Editing Rules

1. **Analysis is pure; I/O stays outside.** `parse_file` takes `source: &str`,
   not a file handle. No `tokio::fs`, no `std::fs` in `query/` or `parser.rs`.
2. **Tree-sitter query changes need golden tests.** Any change to node kind
   matching, field names, or traversal logic in `query/` must have a
   corresponding test in `query/tests.rs` for every affected language.
3. **New language support requires a parser dependency.** Adding a `Language`
   variant also requires the corresponding `tree-sitter-*` crate in `Cargo.toml`
   and an `into_tree_sitter()` mapping in `parser.rs`.
4. **Spans are byte ranges into the original source.** All `Range<usize>` values
   must be derived from `node.start_byte()` / `node.end_byte()`, never
   recomputed or estimated.
5. **Test every language path.** `query/tests.rs` covers Rust, JavaScript,
   Python, and Go. Changes to `find_function_definitions` or `find_calls_to`
   must include coverage for all affected languages.
