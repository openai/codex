// Truncate a &str to a byte budget at a char boundary (prefix)
#[inline]
pub fn take_bytes_at_char_boundary(s: &str, maxb: usize) -> &str {
    if s.len() <= maxb {
        return s;
    }
    let mut last_ok = 0;
    for (i, ch) in s.char_indices() {
        let nb = i + ch.len_utf8();
        if nb > maxb {
            break;
        }
        last_ok = nb;
    }
    &s[..last_ok]
}

// Take a suffix of a &str within a byte budget at a char boundary
#[inline]
pub fn take_last_bytes_at_char_boundary(s: &str, maxb: usize) -> &str {
    if s.len() <= maxb {
        return s;
    }
    let mut start = s.len();
    let mut used = 0usize;
    for (i, ch) in s.char_indices().rev() {
        let nb = ch.len_utf8();
        if used + nb > maxb {
            break;
        }
        start = i;
        used += nb;
        if start == 0 {
            break;
        }
    }
    &s[start..]
}

/// Sanitize a tag value to comply with metric tag validation rules:
/// only ASCII alphanumeric, '.', '_', '-', and '/' are allowed.
pub fn sanitize_metric_tag_value(value: &str) -> String {
    const MAX_LEN: usize = 256;
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() || trimmed.chars().all(|ch| !ch.is_ascii_alphanumeric()) {
        return "unspecified".to_string();
    }
    if trimmed.len() <= MAX_LEN {
        trimmed.to_string()
    } else {
        trimmed[..MAX_LEN].to_string()
    }
}

/// Find all UUIDs in a string.
pub fn find_uuids(s: &str) -> Vec<String> {
    let mut uuids = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;

    let is_hex = |b: u8| b.is_ascii_hexdigit();
    let is_uuid = |start: usize, bytes: &[u8]| -> bool {
        let mut i = start;
        let groups = [8, 4, 4, 4, 12];

        for (group_idx, group_len) in groups.iter().enumerate() {
            for offset in 0..*group_len {
                if i + offset >= bytes.len() || !is_hex(bytes[i + offset]) {
                    return false;
                }
            }
            i += group_len;

            if group_idx < groups.len() - 1 {
                if i >= bytes.len() || bytes[i] != b'-' {
                    return false;
                }
                i += 1;
            }
        }

        true
    };

    while i + 36 <= bytes.len() {
        if is_uuid(i, bytes) {
            uuids.push(s[i..i + 36].to_string());
            i += 36;
            continue;
        }
        i += 1;
    }

    uuids
}

#[cfg(test)]
mod tests {
    use super::find_uuids;
    use super::sanitize_metric_tag_value;
    use pretty_assertions::assert_eq;

    #[test]
    fn find_uuids_finds_multiple() {
        let input =
            "x 00112233-4455-6677-8899-aabbccddeeff-k y 12345678-90ab-cdef-0123-456789abcdef";
        assert_eq!(
            find_uuids(input),
            vec![
                "00112233-4455-6677-8899-aabbccddeeff".to_string(),
                "12345678-90ab-cdef-0123-456789abcdef".to_string(),
            ]
        );
    }

    #[test]
    fn find_uuids_ignores_invalid() {
        let input = "not-a-uuid-1234-5678-9abc-def0-123456789abc";
        assert_eq!(find_uuids(input), Vec::<String>::new());
    }

    #[test]
    fn find_uuids_handles_non_ascii_without_overlap() {
        let input = "🙂 55e5d6f7-8a7f-4d2a-8d88-123456789012abc";
        assert_eq!(
            find_uuids(input),
            vec!["55e5d6f7-8a7f-4d2a-8d88-123456789012".to_string()]
        );
    }

    #[test]
    fn sanitize_metric_tag_value_trims_and_fills_unspecified() {
        let msg = "///";
        assert_eq!(sanitize_metric_tag_value(msg), "unspecified");
    }

    #[test]
    fn sanitize_metric_tag_value_replaces_invalid_chars() {
        let msg = "bad value!";
        assert_eq!(sanitize_metric_tag_value(msg), "bad_value");
    }
}
