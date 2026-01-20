use codex_protocol::custom_prompts::CustomPrompt;
use codex_utils_absolute_path::AbsolutePathBuf;
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

pub async fn discover_layered_prompts_for_cwd(
    cwd: &Path,
    config_layer_stack: &crate::config_loader::ConfigLayerStack,
) -> Vec<CustomPrompt> {
    discover_layered_prompts_for_cwd_with_global(
        cwd,
        config_layer_stack,
        default_prompts_dir().as_deref(),
    )
    .await
}

async fn discover_layered_prompts_for_cwd_with_global(
    cwd: &Path,
    config_layer_stack: &crate::config_loader::ConfigLayerStack,
    global_prompt_dir: Option<&Path>,
) -> Vec<CustomPrompt> {
    let Ok(cwd) = AbsolutePathBuf::from_absolute_path(cwd) else {
        return Vec::new();
    };
    let project_root =
        match crate::config_loader::find_project_root_for_layer_stack(&cwd, config_layer_stack)
            .await
        {
            Ok(root) => root,
            Err(_) => return Vec::new(),
        };

    let project_prompt_dirs = cwd
        .as_path()
        .ancestors()
        .scan(false, |done, ancestor| {
            if *done {
                return None;
            }
            if ancestor == project_root.as_path() {
                *done = true;
            }
            Some(ancestor.join(".codex").join("prompts"))
        })
        .collect::<Vec<_>>();
    discover_layered_prompts_from_dirs(&project_prompt_dirs, global_prompt_dir).await
}

async fn discover_layered_prompts_from_dirs(
    project_prompt_dirs: &[PathBuf],
    global_prompt_dir: Option<&Path>,
) -> Vec<CustomPrompt> {
    let mut out = Vec::new();
    let mut exclude = HashSet::new();

    for dir in project_prompt_dirs {
        let found = discover_prompts_in_excluding(dir, &exclude).await;
        for prompt in &found {
            exclude.insert(prompt.name.clone());
        }
        out.extend(found);
    }

    if let Some(dir) = global_prompt_dir {
        let found = discover_prompts_in_excluding(dir, &exclude).await;
        for prompt in &found {
            exclude.insert(prompt.name.clone());
        }
        out.extend(found);
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
    use pretty_assertions::assert_eq;
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

    #[tokio::test]
    async fn layered_prompts_prefer_deeper_dirs_and_keep_globals_last() {
        let tmp = tempdir().expect("create TempDir");
        let root = tmp.path().join("root");
        let child = root.join("child");
        let grandchild = child.join("grandchild");
        let global = tmp.path().join("global");

        fs::create_dir_all(root.join(".codex/prompts")).unwrap();
        fs::create_dir_all(child.join(".codex/prompts")).unwrap();
        fs::create_dir_all(grandchild.join(".codex/prompts")).unwrap();
        fs::create_dir_all(&global).unwrap();

        fs::write(root.join(".codex/prompts/shared.md"), "root shared").unwrap();
        fs::write(child.join(".codex/prompts/child-only.md"), "child only").unwrap();
        fs::write(child.join(".codex/prompts/shared.md"), "child shared").unwrap();
        fs::write(
            grandchild.join(".codex/prompts/shared.md"),
            "grandchild shared",
        )
        .unwrap();
        fs::write(global.join("shared.md"), "global shared").unwrap();
        fs::write(global.join("global-only.md"), "global only").unwrap();

        let project_prompt_dirs = vec![
            grandchild.join(".codex/prompts"),
            child.join(".codex/prompts"),
            root.join(".codex/prompts"),
        ];

        let prompts = discover_layered_prompts_from_dirs(&project_prompt_dirs, Some(&global)).await;
        let prompt_map: std::collections::HashMap<String, CustomPrompt> = prompts
            .into_iter()
            .map(|prompt| (prompt.name.clone(), prompt))
            .collect();

        assert_eq!(
            prompt_map.get("shared").expect("shared prompt").content,
            "grandchild shared"
        );
        assert_eq!(
            prompt_map
                .get("child-only")
                .expect("child-only prompt")
                .content,
            "child only"
        );
        assert_eq!(
            prompt_map
                .get("global-only")
                .expect("global-only prompt")
                .content,
            "global only"
        );
    }
}
