use tracing::warn;

/// Wrap [`parse_verdict`] with a fail-closed default and a structured warning
/// so the caller does not silently treat an unparseable LLM response as a
/// passing review.
pub(super) fn verdict_pass(reviewer: &str, output: &str) -> bool {
    match parse_verdict(output) {
        Some(verdict) => verdict,
        None => {
            warn!(
                reviewer = reviewer,
                "No VERDICT: PASS|FAIL line found in reviewer output; treating as FAIL"
            );
            false
        }
    }
}

/// Parse the structured verdict from a reviewer's reply.
///
/// We look for `VERDICT:` followed by a `PASS`/`FAIL`-like token within the
/// last 200 characters of the output (case-insensitive). The tail-only scope
/// avoids matching the verdict instruction echoed back in the body. Returns
/// `None` when no recognizable verdict is found, so callers can fail-closed.
pub(super) fn parse_verdict(output: &str) -> Option<bool> {
    let tail = if output.len() > 200 {
        let mut idx = output.len() - 200;
        while !output.is_char_boundary(idx) {
            idx += 1;
        }
        &output[idx..]
    } else {
        output
    };

    let lower = tail.to_lowercase();
    let pos = lower.rfind("verdict:")?;
    let after = &lower[pos + "verdict:".len()..];
    let token = after
        .split_whitespace()
        .next()?
        .trim_end_matches(|c: char| !c.is_alphanumeric());

    match token {
        "pass" | "passed" | "approve" | "approved" | "ok" => Some(true),
        "fail" | "failed" | "reject" | "rejected" | "block" | "blocked" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod verdict_tests {
    use super::parse_verdict;

    #[test]
    fn parses_pass_at_end() {
        assert_eq!(parse_verdict("looks fine.\nVERDICT: PASS"), Some(true));
    }

    #[test]
    fn parses_fail_at_end() {
        assert_eq!(
            parse_verdict("found a sql injection on line 12.\nVERDICT: FAIL"),
            Some(false)
        );
    }

    #[test]
    fn benign_mention_of_fail_does_not_flip_verdict() {
        let body = "Notes: no test failures detected; fail-safe pattern used.\n\
                    VERDICT: PASS";
        assert_eq!(parse_verdict(body), Some(true));
    }

    #[test]
    fn missing_verdict_returns_none() {
        assert_eq!(parse_verdict("looks fine, nothing critical"), None);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(parse_verdict("\nverdict: pass\n"), Some(true));
        assert_eq!(parse_verdict("\nVerdict: Fail\n"), Some(false));
    }

    #[test]
    fn last_verdict_wins_within_tail() {
        let body = "VERDICT: FAIL\n\
                    on second thought, fixed it.\n\
                    VERDICT: PASS";
        assert_eq!(parse_verdict(body), Some(true));
    }

    #[test]
    fn verdict_at_start_of_long_body_is_ignored() {
        // 200-char tail window is the contract; a verdict in the first
        // half of a >400-char body must be invisible to the parser. This
        // locks in the tail semantics: the prompt asks the LLM to put the
        // verdict at the END, and we refuse to honor a stray earlier line.
        let mut body = String::from("VERDICT: PASS\n");
        body.push_str(&"prose ".repeat(60)); // ~360 chars of filler
                                             // No VERDICT at the end → parser sees no verdict in last 200 chars.
        assert_eq!(parse_verdict(&body), None);
    }

    #[test]
    fn multibyte_tail_boundary_is_safe() {
        // 4-byte UTF-8 grapheme straddling the 200-char tail boundary must
        // not panic; the parser walks forward to the next char boundary.
        let pad = "𝓍".repeat(80); // each char is 4 bytes → 320 bytes
        let body = format!("{}\nfinal note.\nVERDICT: PASS", pad);
        assert_eq!(parse_verdict(&body), Some(true));
    }
}
