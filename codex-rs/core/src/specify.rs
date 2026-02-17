//! Specify (spec-kit) command discovery.
//!
//! Detects whether `specify init` has been run in the current project and
//! discovers the available slash-command TOML/Markdown templates installed
//! by spec-kit under the relevant AI agent directory.

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Agent directory configurations
// ---------------------------------------------------------------------------

/// Agent directories that store commands as `.toml` files.
const TOML_AGENT_DIRS: &[&str] = &[".codex/commands", ".gemini/commands", ".qwen/commands"];

/// Agent directories that store commands as `.md` files.
const MD_AGENT_DIRS: &[&str] = &[
    ".codex/prompts",
    ".claude/commands",
    ".cursor/commands",
    ".github/agents",
];

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single Specify command discovered from a TOML or Markdown template file.
#[derive(Debug, Clone)]
pub struct SpecifyCommand {
    /// Command name derived from the file stem (e.g. `"constitution"`).
    pub name: String,
    /// Human-readable description (from the TOML `description` field or MD
    /// frontmatter).
    pub description: String,
    /// The prompt body that should be sent to the model.
    pub prompt: String,
    /// Absolute path to the template file.
    pub path: PathBuf,
}

// ---------------------------------------------------------------------------
// TOML schema (matches spec-kit `_render_toml_command` output)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TomlTemplate {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns `true` if any known AI-agent commands directory exists under `cwd`.
pub fn is_specify_initialized(cwd: &Path) -> bool {
    for dir in TOML_AGENT_DIRS.iter().chain(MD_AGENT_DIRS.iter()) {
        if cwd.join(dir).is_dir() {
            return true;
        }
    }
    false
}

/// Discover all Specify commands available under `cwd`.
///
/// The function scans known agent directories (TOML first, then Markdown),
/// and returns the commands found in the **first** directory that exists.
pub fn discover_specify_commands(cwd: &Path) -> Vec<SpecifyCommand> {
    // 1. Check for .codex specific directories first (Highest Priority)

    // .codex/commands (TOML)
    let codex_toml = cwd.join(".codex/commands");
    if codex_toml.is_dir() {
        return read_toml_commands(&codex_toml);
    }

    // .codex/prompts (MD)
    let codex_md = cwd.join(".codex/prompts");
    if codex_md.is_dir() {
        return read_md_commands(&codex_md);
    }

    // 2. Check other TOML agent directories
    for dir in TOML_AGENT_DIRS {
        if *dir == ".codex/commands" {
            continue;
        }
        let commands_dir = cwd.join(dir);
        if commands_dir.is_dir() {
            return read_toml_commands(&commands_dir);
        }
    }

    // 3. Check other MD agent directories
    for dir in MD_AGENT_DIRS {
        if *dir == ".codex/prompts" {
            continue;
        }
        let commands_dir = cwd.join(dir);
        if commands_dir.is_dir() {
            return read_md_commands(&commands_dir);
        }
    }

    Vec::new()
}

// ---------------------------------------------------------------------------
// TOML parsing
// ---------------------------------------------------------------------------

fn read_toml_commands(dir: &Path) -> Vec<SpecifyCommand> {
    let mut commands = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return commands,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "toml") {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Strip the "speckit." prefix for the command name shown in the TUI.
        let display_name = name.strip_prefix("speckit.").unwrap_or(&name).to_string();

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let template: TomlTemplate = match toml::from_str(&content) {
            Ok(t) => t,
            Err(_) => continue,
        };

        commands.push(SpecifyCommand {
            name: display_name,
            description: template.description.unwrap_or_default(),
            prompt: template.prompt.unwrap_or_default(),
            path,
        });
    }

    commands.sort_by(|a, b| a.name.cmp(&b.name));
    commands
}

// ---------------------------------------------------------------------------
// Markdown parsing (YAML frontmatter)
// ---------------------------------------------------------------------------

fn read_md_commands(dir: &Path) -> Vec<SpecifyCommand> {
    let mut commands = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return commands,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "md") {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Only include speckit command files (prefixed with "speckit.")
        // to avoid picking up unrelated prompts.
        if !name.starts_with("speckit.") {
            continue;
        }

        // Strip the "speckit." prefix for the command name shown in the TUI.
        let display_name = name.strip_prefix("speckit.").unwrap_or(&name).to_string();

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let (description, body) = parse_md_frontmatter(&content);

        commands.push(SpecifyCommand {
            name: display_name,
            description,
            prompt: body,
            path,
        });
    }

    commands.sort_by(|a, b| a.name.cmp(&b.name));
    commands
}

/// Extract YAML frontmatter `description` and the body from a Markdown file.
fn parse_md_frontmatter(content: &str) -> (String, String) {
    if !content.starts_with("---") {
        return (String::new(), content.to_string());
    }

    let after_opening = &content[3..];
    let end_marker = match after_opening.find("---") {
        Some(pos) => pos,
        None => return (String::new(), content.to_string()),
    };

    let frontmatter_str = &after_opening[..end_marker];
    let body = after_opening[end_marker + 3..].trim().to_string();

    // Extract `description:` value with basic YAML parsing.
    let description = frontmatter_str
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("description:") {
                Some(rest.trim().trim_matches('"').trim_matches('\'').to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    (description, body)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_is_specify_initialized_with_codex_commands() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        // Not initialized yet.
        assert!(!is_specify_initialized(root));

        // Create .codex/commands/
        fs::create_dir_all(root.join(".codex/commands")).unwrap();
        assert!(is_specify_initialized(root));
    }

    #[test]
    fn test_is_specify_initialized_with_gemini_commands() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join(".gemini/commands")).unwrap();
        assert!(is_specify_initialized(root));
    }

    #[test]
    fn test_is_specify_initialized_with_claude_commands() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join(".claude/commands")).unwrap();
        assert!(is_specify_initialized(root));
    }

    #[test]
    fn test_discover_toml_commands() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let commands_dir = root.join(".codex/commands");
        fs::create_dir_all(&commands_dir).unwrap();

        fs::write(
            commands_dir.join("constitution.toml"),
            r#"
description = "Create or update the project constitution"

prompt = """
You are a specification assistant.
Update the constitution at .specify/memory/constitution.md.
"""
"#,
        )
        .unwrap();

        fs::write(
            commands_dir.join("plan.toml"),
            r#"
description = "Generate implementation plan"

prompt = """
Create a detailed plan.
"""
"#,
        )
        .unwrap();

        let cmds = discover_specify_commands(root);
        assert_eq!(cmds.len(), 2);

        // Sorted alphabetically
        assert_eq!(cmds[0].name, "constitution");
        assert_eq!(
            cmds[0].description,
            "Create or update the project constitution"
        );
        assert!(cmds[0].prompt.contains("specification assistant"));

        assert_eq!(cmds[1].name, "plan");
        assert_eq!(cmds[1].description, "Generate implementation plan");
    }

    #[test]
    fn test_discover_md_commands() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let commands_dir = root.join(".claude/commands");
        fs::create_dir_all(&commands_dir).unwrap();

        fs::write(
            commands_dir.join("clarify.md"),
            "---\ndescription: Clarify ambiguous requirements\n---\n\nAsk clarifying questions.\n",
        )
        .unwrap();

        let cmds = discover_specify_commands(root);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "clarify");
        assert_eq!(cmds[0].description, "Clarify ambiguous requirements");
        assert!(cmds[0].prompt.contains("clarifying questions"));
    }

    #[test]
    fn test_discover_empty_directory() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let commands_dir = root.join(".codex/commands");
        fs::create_dir_all(&commands_dir).unwrap();

        let cmds = discover_specify_commands(root);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_discover_ignores_invalid_toml() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let commands_dir = root.join(".codex/commands");
        fs::create_dir_all(&commands_dir).unwrap();

        fs::write(
            commands_dir.join("broken.toml"),
            "this is not valid toml {{{{",
        )
        .unwrap();

        fs::write(
            commands_dir.join("valid.toml"),
            "description = \"A valid command\"\nprompt = \"Do something\"\n",
        )
        .unwrap();

        let cmds = discover_specify_commands(root);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "valid");
    }

    #[test]
    fn test_toml_dirs_take_priority_over_md() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        // Create both a TOML and MD commands directory.
        let toml_dir = root.join(".codex/commands");
        let md_dir = root.join(".claude/commands");
        fs::create_dir_all(&toml_dir).unwrap();
        fs::create_dir_all(&md_dir).unwrap();

        fs::write(
            toml_dir.join("a.toml"),
            "description = \"from toml\"\nprompt = \"toml prompt\"\n",
        )
        .unwrap();
        fs::write(
            md_dir.join("b.md"),
            "---\ndescription: from md\n---\nmd prompt\n",
        )
        .unwrap();

        // TOML directory is found first.
        let cmds = discover_specify_commands(root);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "a");
        assert_eq!(cmds[0].description, "from toml");
    }

    #[test]
    fn test_parse_md_frontmatter() {
        let content = "---\ndescription: Create specification\nhandoffs:\n  - label: Next\n---\n\nBody content here.\n";
        let (desc, body) = parse_md_frontmatter(content);
        assert_eq!(desc, "Create specification");
        assert!(body.contains("Body content here."));
    }

    #[test]
    fn test_parse_md_frontmatter_no_frontmatter() {
        let content = "Just plain markdown.\n";
        let (desc, body) = parse_md_frontmatter(content);
        assert!(desc.is_empty());
        assert_eq!(body, content);
    }
}
