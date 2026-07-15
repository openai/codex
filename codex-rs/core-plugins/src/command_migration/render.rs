use super::RewriteProfile;

pub(super) fn rewrite_terms(content: &str, profile: RewriteProfile) -> String {
    let mut rewritten =
        replace_case_insensitive_with_boundaries(content, profile.doc_file_name, "AGENTS.md");
    for from in profile.term_variants {
        rewritten = replace_case_insensitive_with_boundaries(&rewritten, from, "Codex");
    }
    for from in profile.case_sensitive_term_variants {
        rewritten = replace_with_boundaries(&rewritten, from, "Codex");
    }
    rewritten
}

fn replace_with_boundaries(input: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return input.to_string();
    }

    replace_with_boundaries_impl(input, needle, replacement, input)
}

fn replace_case_insensitive_with_boundaries(
    input: &str,
    needle: &str,
    replacement: &str,
) -> String {
    let needle_lower = needle.to_ascii_lowercase();
    if needle_lower.is_empty() {
        return input.to_string();
    }
    let haystack_lower = input.to_ascii_lowercase();
    replace_with_boundaries_impl(input, &needle_lower, replacement, &haystack_lower)
}

fn replace_with_boundaries_impl(
    input: &str,
    needle: &str,
    replacement: &str,
    searchable_input: &str,
) -> String {
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut last_emitted = 0usize;
    let mut search_start = 0usize;

    while let Some(relative_pos) = searchable_input[search_start..].find(needle) {
        let start = search_start + relative_pos;
        let end = start + needle.len();
        let boundary_before = start == 0 || !is_word_byte(bytes[start - 1]);
        let boundary_after = end == bytes.len() || !is_word_byte(bytes[end]);

        if boundary_before && boundary_after {
            output.push_str(&input[last_emitted..start]);
            output.push_str(replacement);
            last_emitted = end;
        }
        search_start = start + 1;
    }

    if last_emitted == 0 {
        return input.to_string();
    }
    output.push_str(&input[last_emitted..]);
    output
}

pub(super) fn yaml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(super) fn slugify_name(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "migrated".to_string()
    } else {
        slug
    }
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
