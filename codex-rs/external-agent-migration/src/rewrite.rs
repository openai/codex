/// Describes source-specific terms that should be rewritten in migrated artifacts.
#[derive(Clone, Copy)]
pub struct RewriteProfile {
    doc_file_name: &'static str,
    term_variants: &'static [&'static str],
    case_sensitive_term_variants: &'static [&'static str],
}

impl RewriteProfile {
    pub const fn new(doc_file_name: &'static str, term_variants: &'static [&'static str]) -> Self {
        Self {
            doc_file_name,
            term_variants,
            case_sensitive_term_variants: &[],
        }
    }

    pub const fn with_case_sensitive_term_variants(
        mut self,
        term_variants: &'static [&'static str],
    ) -> Self {
        self.case_sensitive_term_variants = term_variants;
        self
    }

    pub const fn doc_file_name(self) -> &'static str {
        self.doc_file_name
    }

    pub const fn term_variants(self) -> &'static [&'static str] {
        self.term_variants
    }

    pub const fn case_sensitive_term_variants(self) -> &'static [&'static str] {
        self.case_sensitive_term_variants
    }

    /// Rewrites source-specific documentation names and product terms to their Codex forms.
    pub fn rewrite(self, content: &str) -> String {
        let mut rewritten =
            replace_case_insensitive_with_boundaries(content, self.doc_file_name, "AGENTS.md");
        for from in self.term_variants {
            rewritten = replace_case_insensitive_with_boundaries(&rewritten, from, "Codex");
        }
        for from in self.case_sensitive_term_variants {
            rewritten = replace_with_boundaries(&rewritten, from, "Codex");
        }
        rewritten
    }
}

fn replace_with_boundaries(input: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return input.to_string();
    }

    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut last_emitted = 0usize;
    let mut search_start = 0usize;

    while let Some(relative_pos) = input[search_start..].find(needle) {
        let start = search_start + relative_pos;
        let end = start + needle.len();
        let boundary_before = start == 0 || !is_word_byte(bytes[start - 1]);
        let boundary_after = end == bytes.len() || !is_word_byte(bytes[end]);

        if boundary_before && boundary_after {
            output.push_str(&input[last_emitted..start]);
            output.push_str(replacement);
            last_emitted = end;
        }

        search_start = end;
    }

    if last_emitted == 0 {
        return input.to_string();
    }

    output.push_str(&input[last_emitted..]);
    output
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
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut last_emitted = 0usize;
    let mut search_start = 0usize;

    while let Some(relative_pos) = haystack_lower[search_start..].find(&needle_lower) {
        let start = search_start + relative_pos;
        let end = start + needle_lower.len();
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

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
#[path = "rewrite_tests.rs"]
mod tests;
