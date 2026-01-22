//! Helpers for onboarding guardrail scaffolding.

use crate::git_info::get_git_repo_root;
use std::path::Path;
use std::path::PathBuf;

const AGENTS_TEMPLATE: &str = "# Repository Guidelines\n\n## How to work in this repo\n- Add any key instructions Codex should follow.\n\n## Build and test\n{build_test_section}\n\n## Coding conventions\n- Note formatting, linting, and naming rules.\n\n## Notes for Codex\n- Capture anything that helps Codex work efficiently.\n";

const PLANS_TEMPLATE: &str = "# Plans\n\nUse this file to record approved plans for complex changes.\n\nTemplate\n- Goal\n- Approach\n- Steps\n- Tests\n- Rollback\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardrailScaffoldOutcome {
    pub root: PathBuf,
    pub agents_created: bool,
    pub plans_created: bool,
}

pub fn scaffold_guardrail_files(
    cwd: &Path,
    build_test_commands: Option<&str>,
) -> std::io::Result<GuardrailScaffoldOutcome> {
    let root = get_git_repo_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    let agents_path = root.join("AGENTS.md");
    let plans_path = root.join("PLANS.md");
    let agents_created = write_agents_file(&agents_path, build_test_commands)?;
    let plans_created = write_if_missing(&plans_path, PLANS_TEMPLATE)?;

    Ok(GuardrailScaffoldOutcome {
        root,
        agents_created,
        plans_created,
    })
}

fn write_if_missing(path: &Path, contents: &str) -> std::io::Result<bool> {
    if path.exists() {
        return Ok(false);
    }

    std::fs::write(path, contents)?;
    Ok(true)
}

fn write_agents_file(path: &Path, build_test_commands: Option<&str>) -> std::io::Result<bool> {
    if path.exists() {
        if let Some(commands) = build_test_commands {
            append_build_test_section(path, commands)?;
        }
        return Ok(false);
    }

    let section = format_build_test_section(build_test_commands);
    let contents = AGENTS_TEMPLATE.replace("{build_test_section}", &section);
    std::fs::write(path, contents)?;
    Ok(true)
}

fn append_build_test_section(path: &Path, commands: &str) -> std::io::Result<()> {
    let mut contents = std::fs::read_to_string(path)?;
    if !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push('\n');
    contents.push_str("## Build and test (from onboarding)\n");
    contents.push_str(&format_build_test_section(Some(commands)));
    std::fs::write(path, contents)?;
    Ok(())
}

fn format_build_test_section(commands: Option<&str>) -> String {
    let Some(commands) = commands else {
        return "- List the main build and test commands here.\n".to_string();
    };

    let mut items: Vec<String> = Vec::new();
    for line in commands.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.contains(',') {
            for part in line.split(',') {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    items.push(trimmed.to_string());
                }
            }
        } else {
            items.push(line.to_string());
        }
    }

    if items.is_empty() {
        return "- List the main build and test commands here.\n".to_string();
    }

    let mut out = String::new();
    for item in items {
        out.push_str("- ");
        out.push_str(&item);
        out.push('\n');
    }
    out
}
