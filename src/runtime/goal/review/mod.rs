pub mod architect;
pub mod pass;
pub mod performance;
mod slice;
pub(crate) mod slop;

#[allow(unused_imports)]
pub use pass::{ReviewPass, ReviewPassRegistry};
pub(super) use slice::SliceReviewArtifact;
pub(crate) use slice::{
    anti_slop_confidence_with_findings, review_slice, ANTI_SLOP_ACTIONABLE_THRESHOLD,
};
