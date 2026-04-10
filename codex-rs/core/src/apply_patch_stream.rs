use std::collections::HashMap;
use std::path::PathBuf;

use codex_protocol::protocol::FileChange;
use serde_json::Value;

const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ApplyPatchStreamProgress {
    pub(crate) changes: HashMap<PathBuf, FileChange>,
    pub(crate) active_path: Option<PathBuf>,
}

pub(crate) fn apply_patch_stream_progress(input: &str) -> Option<ApplyPatchStreamProgress> {
    let patch = extract_patch_text(input)?;
    let progress = parse_patch_progress(&patch);
    if progress.changes.is_empty() && progress.active_path.is_none() {
        return None;
    }
    Some(progress)
}

fn extract_patch_text(input: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(input)
        && let Some(patch) = value.get("input").and_then(Value::as_str)
    {
        return Some(patch.to_string());
    }

    if let Some(patch) = extract_json_input_prefix(input)
        && patch.contains(BEGIN_PATCH_MARKER)
    {
        return Some(patch);
    }

    let start = input.find(BEGIN_PATCH_MARKER)?;
    Some(input[start..].to_string())
}

fn extract_json_input_prefix(input: &str) -> Option<String> {
    let key = "\"input\"";
    let mut search_start = 0;
    while let Some(offset) = input[search_start..].find(key) {
        let key_start = search_start + offset;
        let mut index = key_start + key.len();
        index = skip_json_whitespace(input, index);
        if input.as_bytes().get(index) != Some(&b':') {
            search_start = key_start + key.len();
            continue;
        }
        index += 1;
        index = skip_json_whitespace(input, index);
        if input.as_bytes().get(index) != Some(&b'"') {
            search_start = key_start + key.len();
            continue;
        }
        return Some(decode_json_string_prefix(&input[index + 1..]));
    }
    None
}

fn skip_json_whitespace(input: &str, mut index: usize) -> usize {
    while let Some(byte) = input.as_bytes().get(index)
        && byte.is_ascii_whitespace()
    {
        index += 1;
    }
    index
}

fn decode_json_string_prefix(input: &str) -> String {
    let mut decoded = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => break,
            '\\' => {
                let Some(escaped) = chars.next() else {
                    break;
                };
                match escaped {
                    '"' => decoded.push('"'),
                    '\\' => decoded.push('\\'),
                    '/' => decoded.push('/'),
                    'b' => decoded.push('\u{0008}'),
                    'f' => decoded.push('\u{000c}'),
                    'n' => decoded.push('\n'),
                    'r' => decoded.push('\r'),
                    't' => decoded.push('\t'),
                    'u' => {
                        let mut digits = String::with_capacity(4);
                        for _ in 0..4 {
                            let Some(digit) = chars.next() else {
                                return decoded;
                            };
                            digits.push(digit);
                        }
                        if let Ok(value) = u16::from_str_radix(&digits, 16)
                            && let Some(ch) = char::from_u32(u32::from(value))
                        {
                            decoded.push(ch);
                        }
                    }
                    other => decoded.push(other),
                }
            }
            other => decoded.push(other),
        }
    }
    decoded
}

fn parse_patch_progress(patch: &str) -> ApplyPatchStreamProgress {
    let mut changes = HashMap::new();
    let mut current_path: Option<PathBuf> = None;
    let mut mode: Option<PatchSection> = None;
    let mut active_path: Option<PathBuf> = None;

    for segment in patch.split_inclusive('\n') {
        let line = segment.trim_end_matches('\n').trim_end_matches('\r');
        if line == BEGIN_PATCH_MARKER {
            continue;
        }
        if line.starts_with(END_PATCH_MARKER) {
            break;
        }
        if let Some(path) = marker_path(line, "*** Add File:") {
            active_path = Some(path.clone());
            current_path = Some(path.clone());
            mode = Some(PatchSection::Add);
            changes.insert(
                path,
                FileChange::Add {
                    content: String::new(),
                },
            );
            continue;
        }
        if let Some(path) = marker_path(line, "*** Update File:") {
            active_path = Some(path.clone());
            current_path = Some(path.clone());
            mode = Some(PatchSection::Update);
            changes.insert(
                path,
                FileChange::Update {
                    unified_diff: String::new(),
                    move_path: None,
                },
            );
            continue;
        }
        if let Some(path) = marker_path(line, "*** Delete File:") {
            active_path = Some(path.clone());
            current_path = Some(path.clone());
            mode = Some(PatchSection::Delete);
            changes.insert(
                path,
                FileChange::Delete {
                    content: String::new(),
                },
            );
            continue;
        }
        if let Some(move_path) = marker_path(line, "*** Move to:") {
            active_path = Some(move_path.clone());
            if let Some(path) = current_path.as_ref()
                && let Some(FileChange::Update {
                    move_path: existing,
                    ..
                }) = changes.get_mut(path)
            {
                *existing = Some(move_path);
            }
            continue;
        }

        match mode {
            Some(PatchSection::Add) => {
                if let Some(path) = current_path.as_ref()
                    && let Some(FileChange::Add { content }) = changes.get_mut(path)
                    && let Some(added_line) = segment.strip_prefix('+')
                {
                    content.push_str(added_line);
                }
            }
            Some(PatchSection::Update) => {
                if let Some(path) = current_path.as_ref()
                    && let Some(FileChange::Update { unified_diff, .. }) = changes.get_mut(path)
                {
                    unified_diff.push_str(segment);
                }
            }
            Some(PatchSection::Delete) | None => {}
        }
    }

    ApplyPatchStreamProgress {
        changes,
        active_path,
    }
}

fn marker_path(line: &str, marker: &str) -> Option<PathBuf> {
    let path = line.strip_prefix(marker)?.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

#[derive(Clone, Copy)]
enum PatchSection {
    Add,
    Delete,
    Update,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_direct_add_progress() {
        let progress = apply_patch_stream_progress(
            "*** Begin Patch\n*** Add File: src/hello.txt\n+hello\n+wor",
        )
        .expect("patch progress");

        assert_eq!(progress.active_path, Some(PathBuf::from("src/hello.txt")));
        assert_eq!(
            progress.changes,
            HashMap::from([(
                PathBuf::from("src/hello.txt"),
                FileChange::Add {
                    content: "hello\nwor".to_string(),
                },
            )])
        );
    }

    #[test]
    fn decodes_partial_json_arguments() {
        let progress = apply_patch_stream_progress(
            r#"{"input":"*** Begin Patch\n*** Add File: src/hello.txt\n+hello\n+wor"#,
        )
        .expect("patch progress");

        assert_eq!(progress.active_path, Some(PathBuf::from("src/hello.txt")));
        assert_eq!(
            progress.changes,
            HashMap::from([(
                PathBuf::from("src/hello.txt"),
                FileChange::Add {
                    content: "hello\nwor".to_string(),
                },
            )])
        );
    }

    #[test]
    fn parses_update_progress_and_move_path() {
        let progress = apply_patch_stream_progress(
            "*** Begin Patch\n*** Update File: src/old.rs\n*** Move to: src/new.rs\n@@\n-old\n+new",
        )
        .expect("patch progress");

        assert_eq!(progress.active_path, Some(PathBuf::from("src/new.rs")));
        assert_eq!(
            progress.changes,
            HashMap::from([(
                PathBuf::from("src/old.rs"),
                FileChange::Update {
                    unified_diff: "@@\n-old\n+new".to_string(),
                    move_path: Some(PathBuf::from("src/new.rs")),
                },
            )])
        );
    }
}
