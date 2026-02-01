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

/// Return the repo-local prompts directory: `<git root>/.codex/prompts`.
///
/// The git root is detected by walking up from `cwd` until a `.git` marker is found.
/// If no git root is found, returns `None`.
pub fn repo_prompts_dir(cwd: &Path) -> Option<PathBuf> {
    let mut cur = if cwd.is_dir() {
        cwd.to_path_buf()
    } else {
        cwd.parent()?.to_path_buf()
    };

    loop {
        let git_marker = cur.join(".git");
        if std::fs::metadata(&git_marker).is_ok() {
            return Some(cur.join(".codex").join("prompts"));
        }

        let Some(parent) = cur.parent() else {
            return None;
        };
        cur = parent.to_path_buf();
    }
}

/// Discover custom prompts from repo-local and user directories.
///
/// Prompt name conflicts are resolved by preferring repo-local prompts over user prompts.
pub async fn discover_custom_prompts(
    repo_dir: Option<&Path>,
    user_dir: Option<&Path>,
) -> Vec<CustomPrompt> {
    let mut out: Vec<CustomPrompt> = Vec::new();
    let mut exclude: HashSet<String> = HashSet::new();

    if let Some(dir) = repo_dir {
        let prompts = discover_prompts_in(dir).await;
        exclude.extend(prompts.iter().map(|p| p.name.clone()));
        out.extend(prompts);
    }

    if let Some(dir) = user_dir {
        out.extend(discover_prompts_in_excluding(dir, &exclude).await);
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
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
        let is_file_like = fs::metadata(&path)
            .await
            .map(|m| m.is_file())
            .unwrap_or(false);
        if !is_file_like {
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
        let (description, argument_hint, body) = parse_frontmatter(&content);
        out.push(CustomPrompt {
            name,
            path,
            content: body,
            description,
            argument_hint,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Parse optional YAML-like frontmatter at the beginning of `content`.
/// Supported keys:
/// - `description`: short description shown in the slash popup
/// - `argument-hint` or `argument_hint`: brief hint string shown after the description
///   Returns (description, argument_hint, body_without_frontmatter).
fn parse_frontmatter(content: &str) -> (Option<String>, Option<String>, String) {
    let mut segments = content.split_inclusive('\n');
    let Some(first_segment) = segments.next() else {
        return (None, None, String::new());
    };
    let first_line = first_segment.trim_end_matches(['\r', '\n']);
    if first_line.trim() != "---" {
        return (None, None, content.to_string());
    }

    let mut desc: Option<String> = None;
    let mut hint: Option<String> = None;
    let mut frontmatter_closed = false;
    let mut consumed = first_segment.len();

    for segment in segments {
        let line = segment.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim();

        if trimmed == "---" {
            frontmatter_closed = true;
            consumed += segment.len();
            break;
        }

        if trimmed.is_empty() || trimmed.starts_with('#') {
            consumed += segment.len();
            continue;
        }

        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let mut val = v.trim().to_string();
            if val.len() >= 2 {
                let bytes = val.as_bytes();
                let first = bytes[0];
                let last = bytes[bytes.len() - 1];
                if (first == b'\"' && last == b'\"') || (first == b'\'' && last == b'\'') {
                    val = val[1..val.len().saturating_sub(1)].to_string();
                }
            }
            match key.as_str() {
                "description" => desc = Some(val),
                "argument-hint" | "argument_hint" => hint = Some(val),
                _ => {}
            }
        }

        consumed += segment.len();
    }

    if !frontmatter_closed {
        // Unterminated frontmatter: treat input as-is.
        return (None, None, content.to_string());
    }

    let body = if consumed >= content.len() {
        String::new()
    } else {
        content[consumed..].to_string()
    };
    (desc, hint, body)
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
    #[cfg(unix)]
    async fn discovers_symlinked_md_files() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();

        // Create a real file
        fs::write(dir.join("real.md"), b"real content").unwrap();

        // Create a symlink to the real file
        std::os::unix::fs::symlink(dir.join("real.md"), dir.join("link.md")).unwrap();

        let found = discover_prompts_in(dir).await;
        let names: Vec<String> = found.into_iter().map(|e| e.name).collect();

        // Both real and link should be discovered, sorted alphabetically
        assert_eq!(names, vec!["link", "real"]);
    }

    #[tokio::test]
    async fn parses_frontmatter_and_strips_from_body() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();
        let file = dir.join("withmeta.md");
        let text = "---\nname: ignored\ndescription: \"Quick review command\"\nargument-hint: \"[file] [priority]\"\n---\nActual body with $1 and $ARGUMENTS";
        fs::write(&file, text).unwrap();

        let found = discover_prompts_in(dir).await;
        assert_eq!(found.len(), 1);
        let p = &found[0];
        assert_eq!(p.name, "withmeta");
        assert_eq!(p.description.as_deref(), Some("Quick review command"));
        assert_eq!(p.argument_hint.as_deref(), Some("[file] [priority]"));
        // Body should not include the frontmatter delimiters.
        assert_eq!(p.content, "Actual body with $1 and $ARGUMENTS");
    }

    #[test]
    fn parse_frontmatter_preserves_body_newlines() {
        let content = "---\r\ndescription: \"Line endings\"\r\nargument_hint: \"[arg]\"\r\n---\r\nFirst line\r\nSecond line\r\n";
        let (desc, hint, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("Line endings"));
        assert_eq!(hint.as_deref(), Some("[arg]"));
        assert_eq!(body, "First line\r\nSecond line\r\n");
    }

    #[test]
    fn finds_repo_prompts_dir_from_nested_cwd() {
        let tmp = tempdir().expect("create TempDir");
        let repo = tmp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let nested = repo.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();

        assert_eq!(
            repo_prompts_dir(&nested),
            Some(repo.join(".codex").join("prompts"))
        );
    }

    #[tokio::test]
    async fn merges_repo_and_user_prompts_preferring_repo() {
        let tmp = tempdir().expect("create TempDir");

        let repo_prompts = tmp.path().join("repo_prompts");
        fs::create_dir_all(&repo_prompts).unwrap();
        fs::write(repo_prompts.join("shared.md"), b"repo").unwrap();
        fs::write(repo_prompts.join("repo_only.md"), b"repo_only").unwrap();

        let user_prompts = tmp.path().join("user_prompts");
        fs::create_dir_all(&user_prompts).unwrap();
        fs::write(user_prompts.join("shared.md"), b"user").unwrap();
        fs::write(user_prompts.join("user_only.md"), b"user_only").unwrap();

        let found = discover_custom_prompts(Some(&repo_prompts), Some(&user_prompts)).await;
        let names: Vec<String> = found.iter().map(|e| e.name.clone()).collect();
        assert_eq!(names, vec!["repo_only", "shared", "user_only"]);

        let shared = found
            .iter()
            .find(|p| p.name == "shared")
            .expect("shared prompt exists");
        assert_eq!(shared.content, "repo");
    }
}
