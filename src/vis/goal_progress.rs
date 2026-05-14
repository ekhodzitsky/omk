use crate::runtime::goal::GoalProgressSnapshot;

pub fn render_goal_progress(snapshot: &GoalProgressSnapshot) -> String {
    let mut lines = Vec::new();
    lines.push("OMK goal progress".to_string());
    lines.push(format!("phase: {}", snapshot.phase));
    lines.push(format!(
        "current task: {}",
        snapshot.current_task.as_deref().unwrap_or("-")
    ));

    push_section(&mut lines, "done", &snapshot.done, "implemented ", "");
    push_section(&mut lines, "next", &snapshot.next, "", "");
    push_section(&mut lines, "blockers", &snapshot.blockers, "blocked: ", "");
    push_section(
        &mut lines,
        "gates",
        &snapshot.gates,
        "running verification ",
        "",
    );
    push_section(
        &mut lines,
        "reviews",
        &snapshot.reviews,
        "review found blocker ",
        ", creating fix task",
    );

    let proof = snapshot
        .proof_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string());
    lines.push(format!("proof: {proof}"));

    lines.push("narrative:".to_string());
    if snapshot.narrative.is_empty() {
        lines.push("- none".to_string());
    } else {
        lines.extend(
            snapshot
                .narrative
                .iter()
                .map(|line| format!("- {}", line.render())),
        );
    }

    lines.join("\n")
}

fn push_section(
    lines: &mut Vec<String>,
    title: &str,
    entries: &[String],
    prefix: &str,
    suffix: &str,
) {
    lines.push(format!("{title}:"));
    if entries.is_empty() {
        lines.push("- none".to_string());
        return;
    }
    lines.extend(
        entries
            .iter()
            .map(|entry| format!("- {prefix}{entry}{suffix}")),
    );
}
