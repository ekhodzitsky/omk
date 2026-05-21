use anyhow::Result;

const BASE32_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Generate a new session id in the format `o7k_<8-char-base32>`.
///
/// The base32 alphabet excludes I, O, 1, 0 to avoid visual ambiguity.
pub fn new_session_id() -> String {
    let uuid = uuid::Uuid::new_v4();
    let bytes = uuid.as_bytes();

    // Take first 5 bytes (40 bits) → 8 base32 chars.
    let mut bits: u64 = 0;
    for &b in bytes.iter().take(5) {
        bits = (bits << 8) | (b as u64);
    }

    let mut result = String::from("o7k_");
    for i in (0..8).rev() {
        let idx = ((bits >> (i * 5)) & 0x1f) as usize;
        result.push(BASE32_ALPHABET[idx] as char);
    }
    result
}

/// Validate that `s` is a well-formed session id.
pub fn parse_session_id(s: &str) -> Result<()> {
    if !s.starts_with("o7k_") {
        anyhow::bail!("session id must start with 'o7k_'");
    }
    let rest = &s[4..];
    if rest.len() != 8 {
        anyhow::bail!("session id must have exactly 8 chars after 'o7k_'");
    }
    for c in rest.chars() {
        if !BASE32_ALPHABET.contains(&(c as u8)) {
            anyhow::bail!("session id contains invalid character: {}", c);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_has_correct_prefix_and_length() {
        let id = new_session_id();
        assert!(id.starts_with("o7k_"));
        assert_eq!(id.len(), 12); // "o7k_" + 8 chars
    }

    #[test]
    fn parse_valid_id() {
        let id = new_session_id();
        assert!(parse_session_id(&id).is_ok());
    }

    #[test]
    fn parse_rejects_invalid_prefix() {
        assert!(parse_session_id("abc_12345678").is_err());
    }

    #[test]
    fn parse_rejects_bad_char() {
        assert!(parse_session_id("o7k_1234567O").is_err()); // letter O
        assert!(parse_session_id("o7k_1234567I").is_err()); // letter I
        assert!(parse_session_id("o7k_12345671").is_err()); // digit 1
        assert!(parse_session_id("o7k_12345670").is_err()); // digit 0
    }
}
