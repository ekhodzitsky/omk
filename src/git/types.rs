/// Status of a git working tree, parsed from porcelain output.
#[derive(Debug, Clone, Default)]
pub struct GitStatus {
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
}

/// A single entry from `git log`.
#[derive(Debug, Clone)]
pub struct GitLogEntry {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
}

/// A configured git remote.
#[derive(Debug, Clone)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
}

/// Result of a read-only merge-tree operation.
#[derive(Debug, Clone, Default)]
pub struct GitMergeResult {
    pub has_conflicts: bool,
    pub conflict_files: Vec<String>,
    pub tree_oid: Option<String>,
}
