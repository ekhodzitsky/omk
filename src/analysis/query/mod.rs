use std::ops::Range;

use crate::analysis::parser::{Language, SyntaxTree};

mod calls;
mod functions;

/// A function definition found in source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDef {
    pub name: String,
    pub span: Range<usize>,
    pub params: Vec<String>,
}

/// A call site found in source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub name: String,
    pub span: Range<usize>,
}

/// Find all top-level function definitions in a syntax tree.
pub fn find_function_definitions(tree: &SyntaxTree) -> Vec<FunctionDef> {
    functions::collect_functions(&tree.source, tree.tree.root_node(), tree.language)
}

/// Find all calls to a function with the given name.
pub fn find_calls_to(tree: &SyntaxTree, function_name: &str) -> Vec<CallSite> {
    calls::collect_calls(
        &tree.source,
        tree.tree.root_node(),
        tree.language,
        function_name,
    )
}

pub(super) fn node_text(source: &str, node: tree_sitter::Node) -> String {
    source[node.start_byte()..node.end_byte()].to_string()
}

pub(super) fn node_range(node: tree_sitter::Node) -> Range<usize> {
    node.start_byte()..node.end_byte()
}

pub(super) fn extract_params(
    source: &str,
    func_node: tree_sitter::Node,
    language: Language,
) -> Vec<String> {
    let params_field = "parameters";

    let Some(params_node) = func_node.child_by_field_name(params_field) else {
        return Vec::new();
    };

    let kinds: &[&str] = match language {
        Language::Rust => &["parameter", "self_parameter"],
        Language::JavaScript => &[
            "identifier",
            "assignment_pattern",
            "rest_pattern",
            "array_pattern",
            "object_pattern",
        ],
        Language::Python => &[
            "identifier",
            "default_parameter",
            "typed_parameter",
            "typed_default_parameter",
            "list_splat_pattern",
            "dictionary_splat_pattern",
            "tuple_pattern",
        ],
        Language::Go => &["parameter_declaration"],
    };

    let mut params = Vec::new();
    extract_matching_kinds(source, params_node, kinds, &mut params);
    params
}

pub(super) fn extract_matching_kinds(
    source: &str,
    node: tree_sitter::Node,
    kinds: &[&str],
    results: &mut Vec<String>,
) {
    if kinds.contains(&node.kind()) {
        results.push(node_text(source, node).trim().to_string());
        return;
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            extract_matching_kinds(source, child, kinds, results);
        }
    }
}

pub(super) fn call_target_name(source: &str, node: tree_sitter::Node) -> String {
    match node.kind() {
        "identifier" | "field_identifier" | "property_identifier" | "type_identifier" => {
            node_text(source, node)
        }
        "field_expression"
        | "member_expression"
        | "scoped_identifier"
        | "selector_expression"
        | "attribute" => {
            if let Some(last) = node.child_by_field_name("field") {
                node_text(source, last)
            } else if let Some(last) = node.child_by_field_name("name") {
                node_text(source, last)
            } else {
                node_text(source, node)
            }
        }
        _ => node_text(source, node),
    }
}

#[cfg(test)]
mod tests;
