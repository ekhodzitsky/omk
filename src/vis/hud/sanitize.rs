/// Strip ANSI escape sequences and unsafe control bytes from worker- or
/// event-log-supplied text before rendering it to a terminal.
///
/// Without this, a worker that writes `\x1B[2J\x1B[H` (clear screen + home)
/// or OSC sequences into its heartbeat / event payload can take over the
/// HUD's terminal. Tab and newline are preserved so multi-line evidence
/// renders normally.
pub(crate) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1B' {
            match chars.next() {
                Some('[') => {
                    // CSI: 0x40..=0x7E terminates.
                    while let Some(&p) = chars.peek() {
                        chars.next();
                        if matches!(p, '\x40'..='\x7E') {
                            break;
                        }
                    }
                }
                Some(']') | Some('P') | Some('X') | Some('^') | Some('_') => {
                    // String-bracketed sequences: OSC (`]`), DCS (`P`), SOS
                    // (`X`), PM (`^`), APC (`_`). All are terminated by BEL
                    // (0x07) or ESC `\`. Treat their bodies as opaque so an
                    // attacker cannot smuggle visible payload through.
                    while let Some(&p) = chars.peek() {
                        chars.next();
                        if p == '\x07' {
                            break;
                        }
                        if p == '\x1B' {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                Some(_) => {
                    // Two-byte ESC sequence (ESC X) — already consumed second
                    // byte; nothing more to skip.
                }
                None => break,
            }
        } else if c == '\t' || c == '\n' || !c.is_control() {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::strip_ansi;

    #[test]
    fn strip_ansi_removes_csi_clear_screen() {
        // \x1B[2J = ED (Erase in Display), \x1B[H = CUP (Cursor Position).
        // A worker that wrote these into its heartbeat must not be able to
        // wipe the HUD's terminal.
        assert_eq!(strip_ansi("\x1B[2J\x1B[Hhello"), "hello");
    }

    #[test]
    fn strip_ansi_removes_osc_bel_terminated() {
        assert_eq!(strip_ansi("\x1B]0;evil-title\x07ok"), "ok");
    }

    #[test]
    fn strip_ansi_removes_osc_st_terminated() {
        // OSC terminated by ESC \\.
        assert_eq!(strip_ansi("\x1B]0;evil-title\x1B\\ok"), "ok");
    }

    #[test]
    fn strip_ansi_removes_dcs_family() {
        // DCS (ESC P), APC (ESC _), PM (ESC ^), SOS (ESC X). Each must
        // consume its opaque payload, not leak it into the output.
        assert_eq!(strip_ansi("\x1BPpayload\x1B\\after"), "after");
        assert_eq!(strip_ansi("\x1B_apc-data\x07after"), "after");
        assert_eq!(strip_ansi("\x1B^pm-data\x07after"), "after");
        assert_eq!(strip_ansi("\x1BXsos-data\x1B\\after"), "after");
    }

    #[test]
    fn strip_ansi_preserves_tab_and_newline() {
        assert_eq!(strip_ansi("a\tb\nc"), "a\tb\nc");
    }

    #[test]
    fn strip_ansi_strips_bare_control_bytes() {
        assert_eq!(strip_ansi("bell\x07ok"), "bellok");
        assert_eq!(strip_ansi("nul\0ok"), "nulok");
    }

    #[test]
    fn strip_ansi_two_byte_esc_sequence() {
        // ESC M (RI - Reverse Index). Two-byte sequence: drop both.
        assert_eq!(strip_ansi("a\x1BMb"), "ab");
    }

    #[test]
    fn strip_ansi_does_not_panic_on_unterminated_sequence() {
        // Pathological input: ESC at the very end.
        assert_eq!(strip_ansi("trailing\x1B"), "trailing");
        // CSI with no terminator → consumed to EOF, returns empty tail.
        assert_eq!(strip_ansi("a\x1B[999"), "a");
    }

    #[test]
    fn strip_ansi_passes_through_plain_text() {
        assert_eq!(strip_ansi("hello world"), "hello world");
        assert_eq!(strip_ansi("emoji ✅ ok"), "emoji ✅ ok");
    }
}
