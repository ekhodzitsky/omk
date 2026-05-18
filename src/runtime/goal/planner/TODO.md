# planner TODO

## Current
- [x] AST-based file discovery (`discover.rs`) for rs/js/ts/py/go
- [x] Inject discovered files into technical plan artifact
- [ ] Add keyword stemming or synonym expansion to discovery scoring
- [ ] Cache parsed ASTs across multiple goal runs in the same project
- [ ] Add C/C++ and Java tree-sitter grammars

## Next
- [ ] Rank discovered files by call-graph centrality, not just keyword overlap
- [ ] Support `.gitignore`-aware discovery (currently skips target/node_modules only)
- [ ] Add tests for `artifacts.rs` (currently only `discover.rs` has unit tests)
