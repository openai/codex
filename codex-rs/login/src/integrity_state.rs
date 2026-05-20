pub const INTEGRITY_STATE_HEADER_NAME: &str = "X-OAI-IS";
pub const INTEGRITY_STATE_UPDATE_HEADER_NAME: &str = "X-OAI-IS-Update";
pub const INTEGRITY_STATE_TOKEN_RESPONSE_FIELD: &str = "oai_is";
pub const MAX_INTEGRITY_STATE_ENVELOPE_BYTES: usize = 2048;

pub fn is_valid_integrity_state_envelope(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_INTEGRITY_STATE_ENVELOPE_BYTES || value.trim() != value
    {
        return false;
    }

    let mut parts = value.split('.');
    if parts.next() != Some("ois1") {
        return false;
    }

    let valid_part = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    };

    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(header), Some(nonce), Some(ciphertext), None)
            if valid_part(header) && valid_part(nonce) && valid_part(ciphertext)
    )
}

pub fn normalize_integrity_state_envelope(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| is_valid_integrity_state_envelope(value))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_ENVELOPE: &str = "ois1.eyJ2IjoxLCJhbGciOiJBMjU2R0NNIiwia2lkIjoiY2hhdGdwdC13ZWItdjEifQ.ZmFrZW5vbmNlMTIz.ZmFrZWNpcGhlcnRleHQ";

    #[test]
    fn validates_integrity_state_envelopes() {
        assert!(is_valid_integrity_state_envelope(VALID_ENVELOPE));
        assert!(!is_valid_integrity_state_envelope(""));
        assert!(!is_valid_integrity_state_envelope("state"));
        assert!(!is_valid_integrity_state_envelope(&format!(
            " {VALID_ENVELOPE}"
        )));
        assert!(!is_valid_integrity_state_envelope("ois1.a.b"));
        assert!(!is_valid_integrity_state_envelope(&format!(
            "ois1.a.b.{}",
            "c".repeat(MAX_INTEGRITY_STATE_ENVELOPE_BYTES + 1)
        )));
    }
}
