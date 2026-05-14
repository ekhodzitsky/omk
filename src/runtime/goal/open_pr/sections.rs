use serde_json::Value;

use super::{push_blank, push_heading, push_line, string_array, value_str};

pub(super) fn push_review_evidence(body: &mut String, proof_value: &Value) {
    push_heading(body, "Review Evidence");
    let artifacts = proof_value
        .get("review_artifacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if artifacts.is_empty() {
        push_line(body, "- No structured review wall evidence recorded.");
    } else {
        for artifact in artifacts {
            let pass = value_str(artifact, "pass").unwrap_or("unknown");
            let status = value_str(artifact, "status").unwrap_or("unknown");
            let summary = value_str(artifact, "summary").unwrap_or("no summary");
            push_line(body, &format!("- {pass}: {status} - {summary}"));
        }
    }
    push_blank(body);
}

pub(super) fn push_integration_evidence(body: &mut String, proof_value: &Value) {
    push_heading(body, "Integration Evidence");
    let Some(evidence) = proof_value.get("integration_evidence") else {
        push_line(body, "- No integration evidence recorded.");
        push_blank(body);
        return;
    };
    if let Some(status) = value_str(evidence, "status") {
        push_line(body, &format!("- status: {status}"));
    }
    if let Some(summary) = value_str(evidence, "summary") {
        push_line(body, &format!("- summary: {summary}"));
    }
    if let Some(missing) = string_array(evidence.get("missing_evidence")) {
        if !missing.is_empty() {
            push_line(body, "- missing evidence:");
            for gap in missing {
                push_line(body, &format!("  - {gap}"));
            }
        }
    }
    push_blank(body);
}

pub(super) fn push_oracle_evidence(body: &mut String, proof_value: &Value) {
    push_heading(body, "Oracle Evidence");
    let Some(evidence) = proof_value.get("oracle_evidence") else {
        push_line(body, "- No oracle evidence recorded.");
        push_blank(body);
        return;
    };
    if let Some(kind) = value_str(evidence, "kind") {
        push_line(body, &format!("- kind: {kind}"));
    }
    if let Some(status) = value_str(evidence, "status") {
        push_line(body, &format!("- status: {status}"));
    }
    if let Some(checks) = evidence.get("checks").and_then(Value::as_array) {
        push_line(body, "- checks:");
        for check in checks {
            let name = value_str(check, "name").unwrap_or("unknown");
            let status = value_str(check, "status").unwrap_or("unknown");
            let gate = value_str(check, "gate").unwrap_or("no gate");
            push_line(body, &format!("  - {name}: {status} ({gate})"));
        }
    }
    push_blank(body);
}
