use codex_external_agent_migration::RewriteProfile;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub(super) fn display_source_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn copy_dir_recursive(
    source: &Path,
    target: &Path,
    rewrite_profile: RewriteProfile,
) -> io::Result<()> {
    fs::create_dir_all(target)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path, rewrite_profile)?;
            continue;
        }

        if file_type.is_file() {
            if is_skill_md(&source_path) {
                rewrite_and_copy_text_file(&source_path, &target_path, rewrite_profile)?;
            } else {
                fs::copy(source_path, target_path)?;
            }
        }
    }

    Ok(())
}

pub(super) fn rewrite_external_agent_terms(
    content: &str,
    rewrite_profile: RewriteProfile,
) -> String {
    let mut rewritten = replace_case_insensitive_with_boundaries(
        content,
        rewrite_profile.doc_file_name(),
        "AGENTS.md",
    );
    for from in rewrite_profile.term_variants() {
        rewritten = replace_case_insensitive_with_boundaries(&rewritten, from, "Codex");
    }
    for from in rewrite_profile.case_sensitive_term_variants() {
        rewritten = replace_with_boundaries(&rewritten, from, "Codex");
    }
    rewritten
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
        let boundary_before = start == 0 || !is_word_char(bytes[start - 1]);
        let boundary_after = end == bytes.len() || !is_word_char(bytes[end]);
        if boundary_before && boundary_after {
            output.push_str(&input[last_emitted..start]);
            output.push_str(replacement);
            last_emitted = end;
        }
        search_start = end;
    }
    output.push_str(&input[last_emitted..]);
    output
}

fn is_skill_md(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
}

fn rewrite_and_copy_text_file(
    source: &Path,
    target: &Path,
    rewrite_profile: RewriteProfile,
) -> io::Result<()> {
    let source_contents = fs::read_to_string(source)?;
    let rewritten = rewrite_external_agent_terms(&source_contents, rewrite_profile);
    fs::write(target, rewritten)
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
        let boundary_before = start == 0 || !is_word_char(bytes[start - 1]);
        let boundary_after = end == bytes.len() || !is_word_char(bytes[end]);
        if boundary_before && boundary_after {
            output.push_str(&input[last_emitted..start]);
            output.push_str(replacement);
            last_emitted = end;
        }
        search_start = end;
    }
    output.push_str(&input[last_emitted..]);
    output
}

fn is_word_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
