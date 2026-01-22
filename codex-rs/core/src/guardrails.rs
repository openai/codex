//! Helpers for onboarding guardrail scaffolding.

use crate::git_info::get_git_repo_root;
use std::path::Path;
use std::path::PathBuf;

const AGENTS_TEMPLATE: &str = "# Repository Guidelines\n\n## How to work in this repo\n- Add any key instructions Codex should follow.\n\n## Build and test\n- List the main build and test commands here.\n\n## Coding conventions\n- Note formatting, linting, and naming rules.\n\n## Notes for Codex\n- Capture anything that helps Codex work efficiently.\n";

const PLANS_TEMPLATE: &str = "# Plans\n\nUse this file to record approved plans for complex changes.\n\nTemplate\n- Goal\n- Approach\n- Steps\n- Tests\n- Rollback\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardrailScaffoldOutcome {
    pub root: PathBuf,
    pub agents_created: bool,
    pub plans_created: bool,
}

pub fn scaffold_guardrail_files(cwd: &Path) -> std::io::Result<GuardrailScaffoldOutcome> {
    let root = get_git_repo_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    let agents_path = root.join("AGENTS.md");
    let plans_path = root.join("PLANS.md");
    let agents_created = write_if_missing(&agents_path, AGENTS_TEMPLATE)?;
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
