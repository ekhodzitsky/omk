use std::path::Path;

use crate::analysis::parser::parse_file;
use crate::analysis::query::{find_calls_to, find_function_definitions};

// --- Rust tests ---

#[test]
fn test_find_rust_functions() {
    let source = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {
    let x = add(1, 2);
}
"#;
    let tree = parse_file(Path::new("test.rs"), source).unwrap();
    let funcs = find_function_definitions(&tree);
    assert_eq!(funcs.len(), 2);
    assert_eq!(funcs[0].name, "add");
    assert_eq!(funcs[0].params, vec!["a: i32", "b: i32"]);
    assert_eq!(funcs[1].name, "main");
    assert!(funcs[1].params.is_empty());
}

#[test]
fn test_find_rust_calls() {
    let source = r#"
fn add(a: i32, b: i32) -> i32 { a + b }
fn main() {
    let x = add(1, 2);
    let y = add(3, 4);
}
"#;
    let tree = parse_file(Path::new("test.rs"), source).unwrap();
    let calls = find_calls_to(&tree, "add");
    assert_eq!(calls.len(), 2);
}

// --- JavaScript tests ---

#[test]
fn test_find_js_functions() {
    let source = r#"
function greet(name) {
    return "Hello " + name;
}
"#;
    let tree = parse_file(Path::new("test.js"), source).unwrap();
    let funcs = find_function_definitions(&tree);
    assert_eq!(funcs.len(), 1);
    assert_eq!(funcs[0].name, "greet");
    assert_eq!(funcs[0].params, vec!["name"]);
}

#[test]
fn test_find_js_calls() {
    let source = r#"
function greet(name) { return "Hello " + name; }
greet("world");
console.log(greet("test"));
"#;
    let tree = parse_file(Path::new("test.js"), source).unwrap();
    let calls = find_calls_to(&tree, "greet");
    assert_eq!(calls.len(), 2);
}

// --- Python tests ---

#[test]
fn test_find_python_functions() {
    let source = r#"
def add(a, b):
    return a + b
"#;
    let tree = parse_file(Path::new("test.py"), source).unwrap();
    let funcs = find_function_definitions(&tree);
    assert_eq!(funcs.len(), 1);
    assert_eq!(funcs[0].name, "add");
    assert_eq!(funcs[0].params, vec!["a", "b"]);
}

#[test]
fn test_find_python_calls() {
    let source = r#"
def add(a, b):
    return a + b
result = add(1, 2)
print(add(3, 4))
"#;
    let tree = parse_file(Path::new("test.py"), source).unwrap();
    let calls = find_calls_to(&tree, "add");
    assert_eq!(calls.len(), 2);
}

// --- Go tests ---

#[test]
fn test_find_go_functions() {
    let source = r#"
package main

func Add(a int, b int) int {
    return a + b
}
"#;
    let tree = parse_file(Path::new("test.go"), source).unwrap();
    let funcs = find_function_definitions(&tree);
    assert_eq!(funcs.len(), 1);
    assert_eq!(funcs[0].name, "Add");
    assert_eq!(funcs[0].params, vec!["a int", "b int"]);
}

#[test]
fn test_find_go_calls() {
    let source = r#"
package main
func Add(a int, b int) int { return a + b }
func main() {
    x := Add(1, 2)
    y := Add(3, 4)
}
"#;
    let tree = parse_file(Path::new("test.go"), source).unwrap();
    let calls = find_calls_to(&tree, "Add");
    assert_eq!(calls.len(), 2);
}
