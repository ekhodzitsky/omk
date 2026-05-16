use clap::ValueEnum;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub(crate) enum OutputFormat {
    /// Human-readable text (default)
    Text,
    /// Machine-readable JSON
    Json,
    /// Markdown for documentation pipelines
    Md,
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub(crate) enum OpenPrFormat {
    /// Human-readable text
    Text,
    /// Machine-readable JSON
    Json,
    /// Markdown PR title/body draft
    #[value(alias = "md")]
    Markdown,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum OpenPrPolicy {
    /// Render locally without network mutation (default)
    Local,
    /// Create or update a draft GitHub PR
    DraftPr,
    /// Create or update a GitHub PR automatically
    AutoPr,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum MergePolicy {
    /// Do not merge; stop after PR creation
    Disabled,
    /// Stop before merge and record the exact human action needed
    Manual,
    /// Merge only after proof, CI, and required reviews are green
    Gated,
}

pub(crate) fn map_open_pr_policy(policy: OpenPrPolicy) -> crate::runtime::goal::GoalDeliveryPolicy {
    match policy {
        OpenPrPolicy::Local => crate::runtime::goal::GoalDeliveryPolicy::Local,
        OpenPrPolicy::DraftPr => crate::runtime::goal::GoalDeliveryPolicy::DraftPr,
        OpenPrPolicy::AutoPr => crate::runtime::goal::GoalDeliveryPolicy::AutoPr,
    }
}

pub(crate) fn map_merge_policy(policy: MergePolicy) -> crate::runtime::goal::GoalMergePolicy {
    match policy {
        MergePolicy::Disabled => crate::runtime::goal::GoalMergePolicy::Disabled,
        MergePolicy::Manual => crate::runtime::goal::GoalMergePolicy::Manual,
        MergePolicy::Gated => crate::runtime::goal::GoalMergePolicy::Gated,
    }
}
