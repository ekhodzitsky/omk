use anyhow::Result;

pub(in crate::cli::goal) async fn cmd_merge(goal_id: &str) -> Result<()> {
    println!("Goal merge {}", goal_id);
    println!("Merging PR is not yet fully implemented in the standalone merge command.");
    println!(
        "Workaround: use 'omk goal open-pr {} --dry-run --format markdown' to render the PR body,",
        goal_id
    );
    println!("then run 'gh pr merge <pr-url> --squash --delete-branch' manually.");
    println!(
        "For automated merge, use '--merge-policy gated' with 'omk goal run ... --until-ready'."
    );
    Ok(())
}
