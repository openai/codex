use codex_protocol::custom_prompts::CustomPrompt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;

use crate::config::Config;
use crate::git_info::resolve_root_git_project_for_trust;

/// Aggregated result of prompt discovery, including any warnings that should be
/// surfaced to the user (for example, duplicate slash command names).
#[derive(Debug, Default)]
pub struct PromptDiscoveryResult {
    pub prompts: Vec<CustomPrompt>,
    pub warnings: Vec<String>,
}

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
        out.push(CustomPrompt {
            name,
            path,
            content,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Discover prompts for the active session configuration, combining entries
/// from `$CODEX_HOME/prompts` and `<repo-root>/.codex/commands`. Duplicate
/// names are skipped and recorded as warnings.
pub async fn discover_prompts_for_config(config: &Config) -> PromptDiscoveryResult {
    let mut result = PromptDiscoveryResult::default();
    let mut seen: HashMap<String, PathBuf> = HashMap::new();

    let mut lists: Vec<Vec<CustomPrompt>> = Vec::new();

    let mut dirs: Vec<PathBuf> = Vec::new();
    dirs.push(config.codex_home.join("prompts"));
    if let Some(root) = resolve_root_git_project_for_trust(&config.cwd) {
        dirs.push(root.join(".codex/commands"));
    } else {
        dirs.push(config.cwd.join(".codex/commands"));
    }

    for dir in dirs {
        lists.push(discover_prompts_in(&dir).await);
    }

    for list in lists {
        for prompt in list {
            record_prompt(prompt, &mut result, &mut seen);
        }
    }

    result.prompts.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

fn record_prompt(
    prompt: CustomPrompt,
    result: &mut PromptDiscoveryResult,
    seen: &mut HashMap<String, PathBuf>,
) {
    if let Some(existing) = seen.get(&prompt.name) {
        result.warnings.push(format!(
            "duplicate slash command '/{}' defined at {} and {}; skipping later entry",
            prompt.name,
            existing.display(),
            prompt.path.display()
        ));
        return;
    }

    seen.insert(prompt.name.clone(), prompt.path.clone());
    result.prompts.push(prompt);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;

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
    async fn discover_prompts_for_config_merges_sources_and_warns_on_duplicates() {
        let project_dir = tempdir().expect("project tempdir");
        let project_path = project_dir.path();

        // Initialize a git repository so resolve_root_git_project_for_trust succeeds.
        Command::new("git")
            .arg("init")
            .arg("--quiet")
            .current_dir(project_path)
            .status()
            .expect("git init");

        let codex_home_dir = tempdir().expect("codex home tempdir");
        let codex_home = codex_home_dir.path();
        fs::create_dir_all(codex_home.join("prompts")).unwrap();
        fs::write(codex_home.join("prompts/foo.md"), "foo home").unwrap();
        fs::write(codex_home.join("prompts/home-only.md"), "home").unwrap();

        let commands_dir = project_path.join(".codex/commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(commands_dir.join("foo.md"), "foo project").unwrap();
        fs::write(commands_dir.join("task.md"), "do the thing").unwrap();

        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.to_path_buf(),
        )
        .expect("load default config");
        config.cwd = project_path.to_path_buf();

        let result = discover_prompts_for_config(&config).await;

        assert!(result.prompts.iter().any(|p| p.name == "foo"));
        assert!(result.prompts.iter().any(|p| p.name == "home-only"));
        assert!(result.prompts.iter().any(|p| p.name == "task"));
        assert_eq!(result.warnings.len(), 1, "expected duplicate warning");
        assert!(
            result.warnings[0].contains("/foo"),
            "warning should mention duplicate command"
        );
    }

    #[tokio::test]
    async fn project_commands_without_git_repo_are_discovered() {
        let project_dir = tempdir().expect("project tempdir");
        let project_path = project_dir.path();

        let codex_home_dir = tempdir().expect("codex home tempdir");
        let codex_home = codex_home_dir.path();

        fs::create_dir_all(codex_home.join("prompts")).unwrap();
        fs::create_dir_all(project_path.join(".codex/commands")).unwrap();
        fs::write(project_path.join(".codex/commands/test.md"), "project").unwrap();

        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.to_path_buf(),
        )
        .expect("load default config");
        config.cwd = project_path.to_path_buf();

        let result = discover_prompts_for_config(&config).await;
        assert!(result.prompts.iter().any(|p| p.name == "test"));
        assert!(result.warnings.is_empty());
    }
}
