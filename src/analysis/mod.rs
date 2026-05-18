//! Tree-sitter based code analysis module.
//!
//! Provides parsing and AST querying for Rust, JavaScript, Python, and Go.

pub mod parser;
pub mod query;

pub use parser::{parse_file, Language, SyntaxTree};
pub use query::{find_calls_to, find_function_definitions, CallSite, FunctionDef};
