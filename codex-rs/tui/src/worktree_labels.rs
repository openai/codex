use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorktreeLabel {
    pub(crate) name: String,
    pub(crate) branch: Option<String>,
    pub(crate) repo_name: String,
    pub(crate) dirty: bool,
}

impl WorktreeLabel {
    pub(crate) fn summary(&self) -> String {
        let mut parts = vec![self.branch.clone().unwrap_or_else(|| self.name.clone())];
        parts.push(if self.dirty { "dirty" } else { "clean" }.to_string());
        parts.push(self.repo_name.clone());
        parts.join(" · ")
    }
}

pub(crate) fn label_for_cwd(codex_home: &Path, cwd: &Path) -> Option<WorktreeLabel> {
    let info = codex_worktree::resolve_worktree(codex_home, cwd)
        .inspect_err(|err| tracing::warn!(?err, "failed to resolve managed worktree label"))
        .ok()
        .flatten()?;
    Some(WorktreeLabel {
        name: info.name,
        branch: info.branch,
        repo_name: info.repo_name,
        dirty: info.dirty.is_dirty(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn summary_includes_name_branch_and_repo() {
        let label = WorktreeLabel {
            name: String::from("parser-fix"),
            branch: Some(String::from("parser-fix")),
            repo_name: String::from("codex"),
            dirty: false,
        };

        assert_eq!(label.summary(), "parser-fix · clean · codex");
    }
}
