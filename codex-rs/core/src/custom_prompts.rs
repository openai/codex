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

/// Return the project-level prompts directory rooted at the provided `cwd`.
/// The directory does not need to exist; callers should rely on
/// [`discover_prompts_in`] handling missing paths gracefully.
pub fn project_prompts_dir(cwd: &Path) -> PathBuf {
    cwd.join(".codex").join("prompts")
}

/// Discover prompt files in the given directory, returning entries sorted by name.
/// Non-files are ignored. If the directory does not exist or cannot be read, returns empty.
pub async fn discover_prompts_in(dir: &Path) -> Vec<CustomPrompt> {
    discover_prompts_in_excluding(dir, &HashSet::new()).await
}

/// Combine prompts discovered in the project-level and global directories.
/// Prompts from the project-level directory take precedence when names collide.
pub async fn discover_prompts_from_sources(
    project_dir: Option<PathBuf>,
    global_dir: Option<PathBuf>,
) -> Vec<CustomPrompt> {
    let mut seen = HashSet::new();
    let mut out: Vec<CustomPrompt> = Vec::new();

    if let Some(dir) = project_dir {
        for prompt in discover_prompts_in(&dir).await {
            if seen.insert(prompt.name.clone()) {
                out.push(prompt);
            }
        }
    }

    if let Some(dir) = global_dir {
        for prompt in discover_prompts_in(&dir).await {
            if seen.insert(prompt.name.clone()) {
                out.push(prompt);
            }
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
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
        out.push(CustomPrompt {
            name,
            path,
            content,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
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

    #[test]
    fn project_prompts_dir_points_to_dot_codex() {
        let base = Path::new("/workspace/project");
        let expected = base.join(".codex").join("prompts");
        assert_eq!(project_prompts_dir(base), expected);
    }

    #[tokio::test]
    async fn project_prompts_override_global_prompts() {
        let project_tmp = tempdir().expect("project tempdir");
        let project_prompts = project_tmp.path().join(".codex").join("prompts");
        fs::create_dir_all(&project_prompts).unwrap();
        fs::write(project_prompts.join("shared.md"), b"from project").unwrap();
        fs::write(project_prompts.join("project.md"), b"project only").unwrap();

        let global_tmp = tempdir().expect("global tempdir");
        let global_prompts = global_tmp.path();
        fs::write(global_prompts.join("shared.md"), b"from global").unwrap();
        fs::write(global_prompts.join("global.md"), b"global only").unwrap();

        let combined = discover_prompts_from_sources(
            Some(project_prompts.clone()),
            Some(global_prompts.to_path_buf()),
        )
        .await;

        let mut names: Vec<String> = combined.iter().map(|c| c.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["global", "project", "shared"]);

        let shared = combined
            .iter()
            .find(|c| c.name == "shared")
            .expect("shared prompt present");
        assert_eq!(shared.content, "from project");

        let global_only = combined
            .iter()
            .find(|c| c.name == "global")
            .expect("global prompt present");
        assert_eq!(global_only.content, "global only");

        let project_only = combined
            .iter()
            .find(|c| c.name == "project")
            .expect("project prompt present");
        assert_eq!(project_only.content, "project only");
    }

    #[tokio::test]
    async fn combined_prompts_include_global_when_project_missing() {
        let global_tmp = tempdir().expect("global tempdir");
        let global_prompts = global_tmp.path();
        fs::write(global_prompts.join("only-global.md"), b"hello").unwrap();

        let combined =
            discover_prompts_from_sources(None, Some(global_prompts.to_path_buf())).await;
        let names: Vec<String> = combined.into_iter().map(|c| c.name).collect();
        assert_eq!(names, vec!["only-global"]);
    }
}
