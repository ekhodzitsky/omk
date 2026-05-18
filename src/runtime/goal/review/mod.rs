mod slice;

pub use slice::SliceReviewArtifact;
pub(crate) use slice::{anti_slop_confidence, review_slice, ANTI_SLOP_ACTIONABLE_THRESHOLD};
