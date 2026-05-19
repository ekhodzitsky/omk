use crate::analysis::parser::Language;
use crate::analysis::query::{call_target_name, node_range, CallSite};

pub(super) fn collect_calls(
    source: &str,
    root: tree_sitter::Node,
    language: Language,
    target: &str,
) -> Vec<CallSite> {
    let mut results = Vec::new();
    match language {
        Language::Rust => collect_rust_calls(source, root, target, &mut results),
        Language::JavaScript => collect_js_calls(source, root, target, &mut results),
        Language::Python => collect_python_calls(source, root, target, &mut results),
        Language::Go => collect_go_calls(source, root, target, &mut results),
    }
    results
}

fn collect_rust_calls(
    source: &str,
    node: tree_sitter::Node,
    target: &str,
    results: &mut Vec<CallSite>,
) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            let name = call_target_name(source, func);
            if name == target {
                results.push(CallSite {
                    name,
                    span: node_range(node),
                });
            }
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_rust_calls(source, child, target, results);
        }
    }
}

fn collect_js_calls(
    source: &str,
    node: tree_sitter::Node,
    target: &str,
    results: &mut Vec<CallSite>,
) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            let name = call_target_name(source, func);
            if name == target {
                results.push(CallSite {
                    name,
                    span: node_range(node),
                });
            }
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_js_calls(source, child, target, results);
        }
    }
}

fn collect_python_calls(
    source: &str,
    node: tree_sitter::Node,
    target: &str,
    results: &mut Vec<CallSite>,
) {
    if node.kind() == "call" {
        if let Some(func) = node.child_by_field_name("function") {
            let name = call_target_name(source, func);
            if name == target {
                results.push(CallSite {
                    name,
                    span: node_range(node),
                });
            }
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_python_calls(source, child, target, results);
        }
    }
}

fn collect_go_calls(
    source: &str,
    node: tree_sitter::Node,
    target: &str,
    results: &mut Vec<CallSite>,
) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            let name = call_target_name(source, func);
            if name == target {
                results.push(CallSite {
                    name,
                    span: node_range(node),
                });
            }
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_go_calls(source, child, target, results);
        }
    }
}
