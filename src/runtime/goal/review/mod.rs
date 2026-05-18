mod slice;
pub mod slop;

pub use slice::SliceReviewArtifact;
pub(crate) use slice::{
    anti_slop_confidence_with_findings, review_slice, ANTI_SLOP_ACTIONABLE_THRESHOLD,
};
