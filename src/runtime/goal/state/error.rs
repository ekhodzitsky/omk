/// Typed error for goal state loading failures.
///
/// Distinguishes missing files from corrupted or invalid-format state
/// so callers and tests can match on the root cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalStateError {
    MissingFile { path: String },
    IoError { path: String, reason: String },
    InvalidFormat { path: String, reason: String },
}

impl std::fmt::Display for GoalStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFile { path } => write!(f, "Goal state file missing: {path}"),
            Self::IoError { path, reason } => {
                write!(f, "Goal state file unreadable at {path}: {reason}")
            }
            Self::InvalidFormat { path, reason } => {
                write!(f, "Goal state file has invalid format at {path}: {reason}")
            }
        }
    }
}

impl std::error::Error for GoalStateError {}
