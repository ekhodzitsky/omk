use crate::analysis::parser::Language;
use crate::analysis::query::{extract_params, node_range, node_text, FunctionDef};

pub(super) fn collect_functions(
    source: &str,
    root: tree_sitter::Node,
    language: Language,
) -> Vec<FunctionDef> {
    let mut results = Vec::new();
    match language {
        Language::Rust => collect_rust_functions(source, root, &mut results),
        Language::JavaScript => collect_js_functions(source, root, &mut results),
        Language::Python => collect_python_functions(source, root, &mut results),
        Language::Go => collect_go_functions(source, root, &mut results),
    }
    results
}

fn collect_rust_functions(source: &str, node: tree_sitter::Node, results: &mut Vec<FunctionDef>) {
    if node.kind() == "function_item" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = node_text(source, name_node);
            let params = extract_params(source, node, Language::Rust);
            results.push(FunctionDef {
                name,
                span: node_range(node),
                params,
            });
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            collect_rust_functions(source, child, results);
        }
    }
}

fn collect_js_functions(source: &str, node: tree_sitter::Node, results: &mut Vec<FunctionDef>) {
    match node.kind() {
        "function_declaration" | "function_expression" | "method_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(source, name_node);
                let params = extract_params(source, node, Language::JavaScript);
                results.push(FunctionDef {
                    name,
                    span: node_range(node),
                    params,
                });
            }
        }
        "arrow_function" => {
            // Arrow functions may not have a name; skip for now.
        }
        _ => {}
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            collect_js_functions(source, child, results);
        }
    }
}

fn collect_python_functions(source: &str, node: tree_sitter::Node, results: &mut Vec<FunctionDef>) {
    if node.kind() == "function_definition" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = node_text(source, name_node);
            let params = extract_params(source, node, Language::Python);
            results.push(FunctionDef {
                name,
                span: node_range(node),
                params,
            });
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            collect_python_functions(source, child, results);
        }
    }
}

fn collect_go_functions(source: &str, node: tree_sitter::Node, results: &mut Vec<FunctionDef>) {
    if node.kind() == "function_declaration" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = node_text(source, name_node);
            let params = extract_params(source, node, Language::Go);
            results.push(FunctionDef {
                name,
                span: node_range(node),
                params,
            });
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            collect_go_functions(source, child, results);
        }
    }
}
