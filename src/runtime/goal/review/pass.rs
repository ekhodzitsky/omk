use crate::runtime::goal::review::slice::{SliceReviewContext, SliceReviewOutcome};

/// One semantic review pass run against a slice (architect, code,
/// test-engineer, security, performance, anti-slop).
///
/// Each pass is independent and may be added in its own PR without
/// touching this trait or the dispatcher.
#[allow(dead_code)]
pub trait ReviewPass: Send + Sync {
    /// Stable identifier persisted into proof artifacts.
    fn name(&self) -> &'static str;

    /// Run the pass and produce an outcome. Implementations MUST NOT
    /// panic on bad input — return a non-fatal outcome instead.
    fn run(&self, ctx: &SliceReviewContext) -> SliceReviewOutcome;
}

/// Owning registry of review passes used by the slice review dispatcher.
#[allow(dead_code)]
pub struct ReviewPassRegistry {
    passes: Vec<Box<dyn ReviewPass>>,
}

#[allow(dead_code)]
impl ReviewPassRegistry {
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    pub fn register(&mut self, pass: Box<dyn ReviewPass>) {
        self.passes.push(pass);
    }

    pub fn passes(&self) -> &[Box<dyn ReviewPass>] {
        &self.passes
    }
}

#[allow(dead_code)]
impl Default for ReviewPassRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyPass(&'static str);

    impl ReviewPass for DummyPass {
        fn name(&self) -> &'static str {
            self.0
        }

        fn run(&self, _ctx: &SliceReviewContext) -> SliceReviewOutcome {
            SliceReviewOutcome {
                passed: true,
                review_path: None,
                security_review_path: None,
                feedback: None,
                artifacts: Vec::new(),
                slop_findings: Vec::new(),
            }
        }
    }

    #[test]
    fn review_pass_registry_starts_empty() {
        let registry = ReviewPassRegistry::new();
        assert!(registry.passes().is_empty());
    }

    #[test]
    fn review_pass_registry_registers_and_lists_in_order() {
        let mut registry = ReviewPassRegistry::new();
        registry.register(Box::new(DummyPass("architect")));
        registry.register(Box::new(DummyPass("security")));
        let passes = registry.passes();
        assert_eq!(passes.len(), 2);
        assert_eq!(passes[0].name(), "architect");
        assert_eq!(passes[1].name(), "security");
    }
}
