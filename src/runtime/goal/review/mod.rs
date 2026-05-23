pub mod architect;
pub mod dispatcher;
pub mod pass;
pub mod performance;
mod slice;
pub(crate) mod slop;

#[allow(unused_imports)]
pub use pass::{ReviewPass, ReviewPassRegistry};
pub use slice::SliceReviewArtifact;
pub(crate) use slice::SliceReviewOutcome;
pub(crate) use slice::{
    anti_slop_confidence_with_findings, review_slice, ANTI_SLOP_ACTIONABLE_THRESHOLD,
};

#[doc(hidden)]
pub fn test_slice_review_outcome(artifacts: Vec<SliceReviewArtifact>) -> SliceReviewOutcome {
    SliceReviewOutcome {
        passed: artifacts.iter().all(|a| a.passed),
        review_path: None,
        security_review_path: None,
        feedback: None,
        artifacts,
        slop_findings: Vec::new(),
    }
}
