use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::Path;

const REQUIRED_TOP: &[&str] = &[
    "schema_version",
    "module",
    "level",
    "purpose",
    "status",
    "surface",
    "dependencies",
    "consumers",
    "invariants",
    "verification",
];

const VALID_LEVELS: &[&str] = &["root", "subsystem"];
const VALID_STATUSES: &[&str] = &["experimental", "pilot", "stable", "deprecated"];
const VALID_PROOF_KINDS: &[&str] = &[
    "unit-test",
    "integration-test",
    "contract-test",
    "golden",
    "schema",
    "smoke",
    "static-check",
    "benchmark",
    "manual",
    "missing",
];

#[derive(Debug)]
struct Issue {
    path: String,
    message: String,
}

impl Issue {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "  ❌ {}: {}", self.path, self.message)
    }
}

fn main() {
    let mut contracts = 0usize;
    let mut issues: Vec<Issue> = Vec::new();

    for entry in walk_readmes(Path::new("src")) {
        let path = entry;
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                issues.push(Issue::new(
                    path.display().to_string(),
                    format!("read error: {e}"),
                ));
                continue;
            }
        };

        let Some(frontmatter) = extract_frontmatter(&content) else {
            continue;
        };

        contracts += 1;
        println!("\n{}", path.display());

        let doc: serde_yaml::Value = match serde_yaml::from_str(frontmatter) {
            Ok(v) => v,
            Err(e) => {
                issues.push(Issue::new("YAML", format!("parse error: {e}")));
                continue;
            }
        };

        validate_top(&doc, &mut issues);
        validate_surface(&doc, &mut issues);
        validate_dependencies(&doc, &mut issues);
        validate_consumers(&doc, &mut issues);
        validate_invariants(&doc, &mut issues);
        validate_verification(&doc, &mut issues);

        let path_str = path.display().to_string();
        if !issues.iter().any(|i| i.path.starts_with(&path_str)) {
            println!("  ✅ {}: compliant", path_str);
        }
    }

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Total contracts found: {contracts}");
    println!("Total issues: {}", issues.len());

    if issues.is_empty() {
        println!("All contracts valid.");
        std::process::exit(0);
    } else {
        for issue in &issues {
            println!("{issue}");
        }
        println!("Fix issues above to become compliant.");
        std::process::exit(1);
    }
}

fn walk_readmes(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk_readmes(&path));
            } else if path.file_name() == Some(std::ffi::OsStr::new("README.md")) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn extract_frontmatter(content: &str) -> Option<&str> {
    if !content.starts_with("---") {
        return None;
    }
    let rest = &content[3..];
    let end = rest.find("\n---")?;
    let fm = &rest[..end];
    if fm.trim().is_empty() {
        None
    } else {
        Some(fm)
    }
}

fn validate_top(doc: &serde_yaml::Value, issues: &mut Vec<Issue>) {
    let Some(map) = doc.as_mapping() else {
        issues.push(Issue::new("root", "doc must be a mapping"));
        return;
    };
    let keys: HashSet<String> = map
        .keys()
        .filter_map(|k| k.as_str().map(|s| s.to_string()))
        .collect();

    for &key in REQUIRED_TOP {
        if !keys.contains(key) {
            issues.push(Issue::new(key, "missing required field"));
        }
    }

    if let Some(v) = doc.get("schema_version") {
        if v.as_i64() != Some(1) {
            issues.push(Issue::new(
                "schema_version",
                format!("expected 1, got {v:?}"),
            ));
        }
    }

    if let Some(v) = doc.get("level") {
        let val = v.as_str().unwrap_or("");
        if !VALID_LEVELS.contains(&val) {
            issues.push(Issue::new(
                "level",
                format!(
                    "invalid value '{val}' (expected: {})",
                    VALID_LEVELS.join(", ")
                ),
            ));
        }
    }

    if let Some(v) = doc.get("status") {
        let val = v.as_str().unwrap_or("");
        if !VALID_STATUSES.contains(&val) {
            issues.push(Issue::new(
                "status",
                format!(
                    "invalid value '{val}' (expected: {})",
                    VALID_STATUSES.join(", ")
                ),
            ));
        }
    }
}

fn validate_surface(doc: &serde_yaml::Value, issues: &mut Vec<Issue>) {
    let Some(arr) = doc.get("surface").and_then(|v| v.as_sequence()) else {
        issues.push(Issue::new("surface", "must be an array"));
        return;
    };

    for (i, item) in arr.iter().enumerate() {
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let path = format!("surface[{i}]({name})");

        for &key in &["name", "kind", "visibility", "contract", "proof"] {
            if !item
                .as_mapping()
                .map(|m| m.contains_key(key))
                .unwrap_or(false)
            {
                issues.push(Issue::new(&path, format!("missing {key}")));
            }
        }

        if let Some(proof) = item.get("proof") {
            validate_proof(proof, &path, issues);
        }
    }
}

fn validate_proof(proof: &serde_yaml::Value, ctx: &str, issues: &mut Vec<Issue>) {
    let Some(map) = proof.as_mapping() else {
        issues.push(Issue::new(ctx, "proof must be a mapping"));
        return;
    };

    for &key in &["kind", "target"] {
        if !map.contains_key(key) {
            issues.push(Issue::new(format!("{ctx}.proof"), format!("missing {key}")));
        }
    }

    let kind = proof.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    if !VALID_PROOF_KINDS.contains(&kind) {
        issues.push(Issue::new(
            format!("{ctx}.proof"),
            format!("invalid kind '{kind}'"),
        ));
    }

    if kind != "manual" && kind != "missing" {
        let cmd = proof.get("command").and_then(|v| v.as_str()).unwrap_or("");
        if cmd.trim().is_empty() {
            issues.push(Issue::new(
                format!("{ctx}.proof"),
                "command is empty (required unless kind is manual/missing)",
            ));
        }
    }
}

fn validate_dependencies(doc: &serde_yaml::Value, issues: &mut Vec<Issue>) {
    let Some(map) = doc.get("dependencies").and_then(|v| v.as_mapping()) else {
        issues.push(Issue::new("dependencies", "must be a mapping"));
        return;
    };

    if let Some(arr) = map.get("internal").and_then(|v| v.as_sequence()) {
        for (i, item) in arr.iter().enumerate() {
            let path = format!("dependencies.internal[{i}]");
            for &key in &["module", "scope", "reason"] {
                if !item
                    .as_mapping()
                    .map(|m| m.contains_key(key))
                    .unwrap_or(false)
                {
                    issues.push(Issue::new(&path, format!("missing {key}")));
                }
            }
        }
    }

    if let Some(arr) = map.get("external").and_then(|v| v.as_sequence()) {
        for (i, item) in arr.iter().enumerate() {
            let path = format!("dependencies.external[{i}]");
            for &key in &["name", "scope", "reason"] {
                if !item
                    .as_mapping()
                    .map(|m| m.contains_key(key))
                    .unwrap_or(false)
                {
                    issues.push(Issue::new(&path, format!("missing {key}")));
                }
            }
        }
    }
}

fn validate_consumers(doc: &serde_yaml::Value, issues: &mut Vec<Issue>) {
    let Some(arr) = doc.get("consumers").and_then(|v| v.as_sequence()) else {
        issues.push(Issue::new("consumers", "must be an array"));
        return;
    };

    for (i, item) in arr.iter().enumerate() {
        let path = format!("consumers[{i}]");
        if !item
            .as_mapping()
            .map(|m| m.contains_key("path"))
            .unwrap_or(false)
        {
            issues.push(Issue::new(&path, "missing 'path'"));
        }
    }
}

fn validate_invariants(doc: &serde_yaml::Value, issues: &mut Vec<Issue>) {
    let Some(arr) = doc.get("invariants").and_then(|v| v.as_sequence()) else {
        issues.push(Issue::new("invariants", "must be an array"));
        return;
    };

    for (i, item) in arr.iter().enumerate() {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let path = format!("invariants[{i}]({id})");

        for &key in &["id", "rule", "proof"] {
            if !item
                .as_mapping()
                .map(|m| m.contains_key(key))
                .unwrap_or(false)
            {
                issues.push(Issue::new(&path, format!("missing {key}")));
            }
        }

        if let Some(proof) = item.get("proof") {
            validate_proof(proof, &path, issues);
        }
    }
}

fn validate_verification(doc: &serde_yaml::Value, issues: &mut Vec<Issue>) {
    let Some(map) = doc.get("verification").and_then(|v| v.as_mapping()) else {
        issues.push(Issue::new("verification", "must be a mapping"));
        return;
    };

    for &key in &["pre_change", "full"] {
        if !map.contains_key(key) {
            issues.push(Issue::new("verification", format!("missing {key}")));
        } else if map[key].as_sequence().is_none() {
            issues.push(Issue::new(
                format!("verification.{key}"),
                "must be an array",
            ));
        }
    }
}
