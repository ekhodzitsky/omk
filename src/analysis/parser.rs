use std::path::Path;

use anyhow::{anyhow, Result};

/// A parsed syntax tree with its source and detected language.
#[derive(Debug)]
pub struct SyntaxTree {
    pub tree: tree_sitter::Tree,
    pub source: String,
    pub language: Language,
}

/// Supported programming languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    JavaScript,
    Python,
    Go,
}

impl Language {
    /// Detect language from a file path by extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => Some(Language::Rust),
            Some("js") | Some("jsx") | Some("mjs") | Some("ts") | Some("tsx") => {
                Some(Language::JavaScript)
            }
            Some("py") | Some("pyi") => Some(Language::Python),
            Some("go") => Some(Language::Go),
            _ => None,
        }
    }
}

impl Language {
    fn into_tree_sitter(self) -> tree_sitter::Language {
        match self {
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
            Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::Go => tree_sitter_go::LANGUAGE.into(),
        }
    }
}

/// Parse source code for a given file path.
///
/// Language is inferred from the file extension.
pub fn parse_file(path: &Path, source: &str) -> Result<SyntaxTree> {
    let language = Language::from_path(path)
        .ok_or_else(|| anyhow!("unsupported language for path: {}", path.display()))?;

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language.into_tree_sitter())
        .map_err(|e| anyhow!("failed to set parser language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("failed to parse source"))?;

    Ok(SyntaxTree {
        tree,
        source: source.to_string(),
        language,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust() {
        assert_eq!(
            Language::from_path(Path::new("foo.rs")),
            Some(Language::Rust)
        );
    }

    #[test]
    fn test_detect_js() {
        assert_eq!(
            Language::from_path(Path::new("foo.js")),
            Some(Language::JavaScript)
        );
    }

    #[test]
    fn test_detect_python() {
        assert_eq!(
            Language::from_path(Path::new("foo.py")),
            Some(Language::Python)
        );
    }

    #[test]
    fn test_detect_go() {
        assert_eq!(Language::from_path(Path::new("foo.go")), Some(Language::Go));
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(Language::from_path(Path::new("foo.txt")), None);
    }

    #[test]
    fn test_parse_rust() {
        let source = "fn main() {}\n";
        let tree = parse_file(Path::new("test.rs"), source).unwrap();
        assert_eq!(tree.language, Language::Rust);
        assert!(!tree.tree.root_node().has_error());
    }

    #[test]
    fn test_parse_javascript() {
        let source = "function main() {}\n";
        let tree = parse_file(Path::new("test.js"), source).unwrap();
        assert_eq!(tree.language, Language::JavaScript);
        assert!(!tree.tree.root_node().has_error());
    }

    #[test]
    fn test_parse_python() {
        let source = "def main():\n    pass\n";
        let tree = parse_file(Path::new("test.py"), source).unwrap();
        assert_eq!(tree.language, Language::Python);
        assert!(!tree.tree.root_node().has_error());
    }

    #[test]
    fn test_parse_go() {
        let source = "func main() {}\n";
        let tree = parse_file(Path::new("test.go"), source).unwrap();
        assert_eq!(tree.language, Language::Go);
        assert!(!tree.tree.root_node().has_error());
    }

    #[test]
    fn test_parse_invalid_language() {
        let result = parse_file(Path::new("test.txt"), "hello");
        assert!(result.is_err());
    }

    #[test]
    fn parser_does_not_panic_on_empty_source() {
        let _ = parse_file(Path::new("test.rs"), "");
    }

    #[test]
    fn parser_does_not_panic_on_malformed_source() {
        let _ = parse_file(Path::new("test.rs"), "fn {{{{ broken");
    }

    #[test]
    fn parser_does_not_panic_on_binary_garbage() {
        let _ = parse_file(Path::new("test.rs"), "\x00\x01\x02\x03\u{00ff}");
    }

    #[test]
    fn parser_does_not_panic_on_very_long_line() {
        let source = "a".repeat(10_000);
        let _ = parse_file(Path::new("test.py"), &source);
    }

    #[test]
    fn parser_does_not_panic_on_deep_nesting() {
        let source = "{".repeat(500) + &"}".repeat(500);
        let _ = parse_file(Path::new("test.js"), &source);
    }
}
