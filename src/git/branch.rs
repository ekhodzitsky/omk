use crate::git::error::GitError;

/// A typed local branch name with basic validation.
#[derive(Debug, Clone)]
pub struct GitBranch {
    name: String,
}

impl GitBranch {
    /// Create a new branch reference, validating the name.
    pub fn new(name: impl Into<String>) -> Result<Self, GitError> {
        let name = name.into();
        if name.is_empty() {
            return Err(GitError::Parse("branch name is empty".to_string()));
        }
        if name.contains("..") || name.starts_with('-') || name.contains(' ') {
            return Err(GitError::Parse(format!(
                "branch name contains invalid characters: {name}"
            )));
        }
        Ok(Self { name })
    }

    /// Borrow the branch name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Check whether the name looks like a remote-tracking branch.
    pub fn is_remote(&self) -> bool {
        self.name.contains('/')
    }
}

impl AsRef<str> for GitBranch {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

impl From<GitBranch> for String {
    fn from(b: GitBranch) -> Self {
        b.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_branch() {
        let b = GitBranch::new("feature/foo").unwrap();
        assert_eq!(b.name(), "feature/foo");
    }

    #[test]
    fn test_empty_branch() {
        let err = GitBranch::new("").unwrap_err();
        assert!(matches!(err, GitError::Parse(_)));
    }

    #[test]
    fn test_invalid_branch() {
        let err = GitBranch::new("-foo").unwrap_err();
        assert!(matches!(err, GitError::Parse(_)));
    }

    #[test]
    fn test_is_remote() {
        assert!(GitBranch::new("origin/main").unwrap().is_remote());
        assert!(!GitBranch::new("main").unwrap().is_remote());
    }
}
