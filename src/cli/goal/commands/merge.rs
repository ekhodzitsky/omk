use anyhow::Result;

pub(in crate::cli::goal) async fn cmd_merge(goal_id: &str) -> Result<()> {
    println!("Goal merge {}", goal_id);
    println!("Merging PR is not yet fully implemented.");
    Ok(())
}
