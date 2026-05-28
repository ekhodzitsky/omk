//! Tree-sitter based code analysis module.
//!
//! Provides parsing and AST querying for Rust, JavaScript, Python, and Go.

pub mod parser;
pub mod query;

pub use parser::parse_file;
pub use query::find_function_definitions;
