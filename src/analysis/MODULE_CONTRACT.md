# Module Contract: `analysis`

## Surfaces
- `parse_file(path, source) -> Result<SyntaxTree>`
- `find_function_definitions(tree) -> Vec<FunctionDef>`
- `find_calls_to(tree, name) -> Vec<CallSite>`

## Consumers
- Future scope detection (complementing regex-based grep)
- Future call graph construction
- Future `omk goal` context gathering

## Invariants
- `SyntaxTree` owns the source string and the parse tree
- All spans are byte ranges into the original source
- Language is inferred from file extension only
- I/O happens at the edge: this module does not read files

## Dependencies
- `tree-sitter` core
- `tree-sitter-rust`, `tree-sitter-javascript`, `tree-sitter-python`, `tree-sitter-go`
- `anyhow` for error handling
