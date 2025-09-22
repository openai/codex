use codex_protocol::custom_prompts::CustomPrompt;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;

/// Return the default prompts directory: `$CODEX_HOME/prompts`.
/// If `CODEX_HOME` cannot be resolved, returns `None`.
pub fn default_prompts_dir() -> Option<PathBuf> {
    crate::config::find_codex_home()
        .ok()
        .map(|home| home.join("prompts"))
}

/// Discover prompt files in the given directory, returning entries sorted by name.
/// Non-files are ignored. If the directory does not exist or cannot be read, returns empty.
pub async fn discover_prompts_in(dir: &Path) -> Vec<CustomPrompt> {
    discover_prompts_in_excluding(dir, &HashSet::new()).await
}

/// Discover prompt files in the given directory, excluding any with names in `exclude`.
/// Returns entries sorted by name. Non-files are ignored. Missing/unreadable dir yields empty.
pub async fn discover_prompts_in_excluding(
    dir: &Path,
    exclude: &HashSet<String>,
) -> Vec<CustomPrompt> {
    let mut out: Vec<CustomPrompt> = Vec::new();
    let mut entries = match fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(_) => return out,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let is_file = entry
            .file_type()
            .await
            .map(|ft| ft.is_file())
            .unwrap_or(false);
        if !is_file {
            continue;
        }
        // Only include Markdown files with a .md extension.
        let is_md = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if !is_md {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if exclude.contains(&name) {
            continue;
        }
        let content = match fs::read_to_string(&path).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        // Extract optional description from simple front matter and strip it from the body.
        // We support a minimal subset:
        // ---\n
        // description: <single line>\n
        // ---\n
        let (description, body) = parse_front_matter_description_and_body(&content);
        out.push(CustomPrompt {
            name,
            path,
            content: body,
            description,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Parse a minimal YAML-like front matter from the beginning of `content` and
/// return `(description, body_without_front_matter)`.
///
/// Front matter is recognized only when the very first line is exactly `---` (ignoring
/// surrounding whitespace), and it ends at the next line that is exactly `---`.
/// Within the block, if a line with `description:` is present, its value is captured
/// (with surrounding quotes stripped) and returned.
/// If front matter is malformed (no closing `---`), returns `(None, content.to_string())`.
fn parse_front_matter_description_and_body(content: &str) -> (Option<String>, String) {
    let mut lines_iter = content.lines();
    match lines_iter.next() {
        Some(first) if first.trim() == "---" => {}
        _ => return (None, content.to_string()),
    }

    let mut desc: Option<String> = None;
    let mut in_front = true;
    let mut body_lines: Vec<&str> = Vec::new();

    for line in content.lines().skip(1) {
        let trimmed = line.trim();
        if in_front {
            if trimmed == "---" {
                in_front = false;
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("description:") {
                let mut val = rest.trim().to_string();
                if (val.starts_with('"') && val.ends_with('"'))
                    || (val.starts_with('\'') && val.ends_with('\''))
                {
                    val = val[1..val.len().saturating_sub(1)].to_string();
                }
                if !val.is_empty() {
                    desc = Some(val);
                }
            }
        } else {
            body_lines.push(line);
        }
    }

    if in_front {
        // No closing '---'
        return (None, content.to_string());
    }

    let mut body = body_lines.join("\n");
    if content.ends_with('\n') && (!body.is_empty()) {
        body.push('\n');
    }
    (desc, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn empty_when_dir_missing() {
        let tmp = tempdir().expect("create TempDir");
        let missing = tmp.path().join("nope");
        let found = discover_prompts_in(&missing).await;
        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn discovers_and_sorts_files() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();
        fs::write(dir.join("b.md"), b"b").unwrap();
        fs::write(dir.join("a.md"), b"a").unwrap();
        fs::create_dir(dir.join("subdir")).unwrap();
        let found = discover_prompts_in(dir).await;
        let names: Vec<String> = found.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn excludes_builtins() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();
        fs::write(dir.join("init.md"), b"ignored").unwrap();
        fs::write(dir.join("foo.md"), b"ok").unwrap();
        let mut exclude = HashSet::new();
        exclude.insert("init".to_string());
        let found = discover_prompts_in_excluding(dir, &exclude).await;
        let names: Vec<String> = found.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["foo"]);
    }

    #[tokio::test]
    async fn skips_non_utf8_files() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();
        // Valid UTF-8 file
        fs::write(dir.join("good.md"), b"hello").unwrap();
        // Invalid UTF-8 content in .md file (e.g., lone 0xFF byte)
        fs::write(dir.join("bad.md"), vec![0xFF, 0xFE, b'\n']).unwrap();
        let found = discover_prompts_in(dir).await;
        let names: Vec<String> = found.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["good"]);
    }

    #[tokio::test]
    async fn parses_front_matter_description() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();
        let content = b"---\ndescription: Hello world\n---\nBody text";
        fs::write(dir.join("desc.md"), content).unwrap();
        let found = discover_prompts_in(dir).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "desc");
        assert_eq!(found[0].description.as_deref(), Some("Hello world"));
        assert_eq!(found[0].content.as_str(), "Body text");
    }
}
