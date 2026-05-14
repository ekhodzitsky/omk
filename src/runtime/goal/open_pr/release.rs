use crate::runtime::goal::{GoalProof, GoalState, GoalStatus};

pub(super) fn push_release_candidate_notes(
    body: &mut String,
    state: &GoalState,
    proof: &GoalProof,
    draft: bool,
) {
    super::push_heading(body, "Release Candidate Notes");
    super::push_line(body, &format!("- Draft: `{draft}`"));
    super::push_line(
        body,
        "- GitHub mutation: not performed by `omk goal open-pr`.",
    );
    super::push_line(
        body,
        &format!("- merge recommendation: {}", merge_recommendation(proof)),
    );
    super::push_line(
        body,
        &format!(
            "- Release source: local goal proof `{}` in `{}`.",
            proof.goal_id,
            state.state_dir.display()
        ),
    );
    super::push_blank(body);
}

fn merge_recommendation(proof: &GoalProof) -> &'static str {
    if proof.status == GoalStatus::Ready && proof.known_gaps.is_empty() {
        "ready for explicit human merge review"
    } else if proof.status == GoalStatus::Ready {
        "review known gaps before merge"
    } else {
        "keep as draft until proof reaches ready"
    }
}
