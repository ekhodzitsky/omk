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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_state_error_missing_file_display() {
        let err = GoalStateError::MissingFile {
            path: "/tmp/state.json".to_string(),
        };
        assert_eq!(err.to_string(), "Goal state file missing: /tmp/state.json");
    }

    #[test]
    fn goal_state_error_io_error_display() {
        let err = GoalStateError::IoError {
            path: "/tmp/state.json".to_string(),
            reason: "permission denied".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Goal state file unreadable at /tmp/state.json: permission denied"
        );
    }

    #[test]
    fn goal_state_error_invalid_format_display() {
        let err = GoalStateError::InvalidFormat {
            path: "/tmp/state.json".to_string(),
            reason: "bad json".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Goal state file has invalid format at /tmp/state.json: bad json"
        );
    }

    #[test]
    fn goal_state_error_implements_error() {
        let err: Box<dyn std::error::Error> = Box::new(GoalStateError::MissingFile {
            path: "p".to_string(),
        });
        assert!(err.to_string().contains("Goal state file missing"));
    }
}
