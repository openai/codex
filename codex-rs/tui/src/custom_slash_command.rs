//! Support for custom slash commands defined as Markdown files on disk.
//!
//! The search paths and naming conventions mirror the documentation in
//! `docs/slash_command_plan.md`:
//!
//! * Project-scoped commands live under `.codex/commands/` inside the current
//!   working directory and are invoked using the `/project:` prefix.
//! * Personal commands live under `~/.codex/commands/` and use the `/user:`
//!   prefix.
//!
//! Command names are derived from the relative file path:
//!
//! ```text
//! .codex/commands/fix-issue.md          -> /project:fix-issue
//! .codex/commands/review/security.md    -> /project:review__security
//! ~/.codex/commands/review/security.md  -> /user:review__security
//! ```
//!
//! When invoked the contents of the Markdown file are read and every
//! occurrence of `$ARGUMENTS` is replaced with the raw argument string that
//! follows the command.

use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Attempt to expand a user-supplied slash command. If the command corresponds
/// to a custom prompt file this returns `Some(prompt)` where `prompt` is the
/// file contents after placeholder substitution. Otherwise returns `None`.
///
/// `cwd` must be the repository root so we can locate `.codex/commands`.
pub fn expand_custom_command(input: &str, cwd: &Path) -> Option<String> {
    let input = input.trim();
    // Quick bailout: must start with '/'.
    if !input.starts_with('/') {
        return None;
    }

    // Regex-like manual parsing to avoid pulling in heavy dependencies.
    // Format: /<scope:>?<name> [args]
    let mut parts = input[1..].splitn(2, ' ');
    let first_token = parts.next()?; // guaranteed non-empty
    let args = parts.next().unwrap_or("");

    let (scope, cmd_name) = if let Some(idx) = first_token.find(':') {
        (&first_token[..idx], &first_token[idx + 1..])
    } else {
        ("project", first_token)
    };

    // Only project and user scopes are handled.
    let root: PathBuf = match scope {
        // For project scope we only look at the *current* working directory.
        // Users are expected to launch Codex from the project root where the
        // `.codex/commands` directory resides.
        "project" => cwd.join(".codex/commands"),
        "user" => {
            let home = env::var("HOME").ok().map(PathBuf::from)?;
            home.join(".codex/commands")
        }
        _ => return None, // Unknown scope.
    };

    // Convert cmd_name: replace __ with path separators and append .md
    let relative_path = cmd_name.replace("__", std::path::MAIN_SEPARATOR_STR) + ".md";
    let file_path = root.join(relative_path);

    // Security: ensure file path is within root.
    if !file_path.starts_with(&root) {
        return None;
    }

    // Read file. If it does not exist -> not a custom command.
    let contents = fs::read_to_string(&file_path).ok()?;

    // Replace $ARGUMENTS placeholder.
    let prompt = contents.replace("$ARGUMENTS", args);

    Some(prompt)
}

/// Recursively discover all custom command Markdown files in both project and
/// user scopes and return their *slash* names without the leading '/'. The
/// returned strings include the scope prefix (e.g. `project:foo`,
/// `user:bar__baz`).
pub fn discover_custom_commands() -> Vec<String> {
    fn gather(root: &Path, scope: &str, out: &mut Vec<String>) {
        if !root.exists() {
            return;
        }
        // Walk the directory recursively. Use a simple stack to avoid adding
        // the walkdir dependency.
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if path.extension().map(|ext| ext == "md").unwrap_or(false) {
                        if let Ok(rel) = path.strip_prefix(root) {
                            // Build command name.
                            if let Some(stem) = rel.to_str() {
                                let mut cmd = stem
                                    .trim_end_matches(".md")
                                    .replace(std::path::MAIN_SEPARATOR, "__");
                                cmd.make_ascii_lowercase();
                                out.push(format!("{scope}:{cmd}"));
                            }
                        }
                    }
                }
            }
        }
    }

    let mut commands = Vec::new();

    if let Ok(cwd) = env::current_dir() {
        gather(&cwd.join(".codex/commands"), "project", &mut commands);
    }

    if let Ok(home) = env::var("HOME") {
        gather(
            &PathBuf::from(home).join(".codex/commands"),
            "user",
            &mut commands,
        );
    }

    commands
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper to write a markdown file with given relative path inside root.
    fn write_md(root: &Path, rel: &str, content: &str) -> PathBuf {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_expand_project_command() {
        let project_dir = TempDir::new().unwrap();
        let commands_dir = project_dir.path().join(".codex/commands");
        write_md(&commands_dir, "fix.md", "Fixing $ARGUMENTS now!");

        // No explicit scope prefix -> defaults to project.
        let input = "/fix missing tests";
        let expanded = expand_custom_command(input, project_dir.path()).unwrap();
        assert_eq!(expanded, "Fixing missing tests now!");
    }

    #[test]
    fn test_expand_user_command() {
        let home_dir = TempDir::new().unwrap();
        let user_commands_dir = home_dir.path().join(".codex/commands");
        write_md(
            &user_commands_dir,
            "review/security.md",
            "Security review: $ARGUMENTS",
        );

        // Temporarily override HOME for this test.
        // Setting HOME for the duration of the test. Marked unsafe in edition 2024.
        unsafe {
            std::env::set_var("HOME", home_dir.path());
        }

        let cwd = Path::new("/"); // cwd is irrelevant for user scope here.
        let input = "/user:review__security critical module";
        let expanded = expand_custom_command(input, cwd).unwrap();
        assert_eq!(expanded, "Security review: critical module");
    }

    #[test]
    fn test_discover_commands() {
        let project_dir = TempDir::new().unwrap();
        let commands_dir = project_dir.path().join(".codex/commands");
        write_md(&commands_dir, "a.md", "A");
        write_md(&commands_dir, "nested/b.md", "B");

        let home_dir = TempDir::new().unwrap();
        let user_commands_dir = home_dir.path().join(".codex/commands");
        write_md(&user_commands_dir, "c.md", "C");

        // Override env vars so discover_custom_commands sees our dirs.
        std::env::set_current_dir(project_dir.path()).unwrap();
        unsafe {
            std::env::set_var("HOME", home_dir.path());
        }

        let mut commands = discover_custom_commands();
        commands.sort();

        let expected = vec![
            "project:a".to_string(),
            "project:nested__b".to_string(),
            "user:c".to_string(),
        ];
        assert_eq!(commands, expected);
    }
}
