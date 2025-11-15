use std::cmp::min;

/// Build the review prompt and user-facing hint for uncommitted changes.
pub fn review_uncommitted_prompt() -> (String, String) {
    let prompt = "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.".to_string();
    let hint = "current changes".to_string();
    (prompt, hint)
}

/// Build the review prompt and hint for reviewing against a base branch.
pub fn review_branch_prompt(branch: &str) -> (String, String) {
    let prompt = format!(
        "Review the code changes against the base branch '{branch}'. Start by finding the merge diff between the current branch and {branch}'s upstream e.g. (`git merge-base HEAD \"$(git rev-parse --abbrev-ref \"{branch}@{{upstream}}\")\"`), then run `git diff` against that SHA to see what changes we would merge into the {branch} branch. Provide prioritized, actionable findings."
    );
    let hint = format!("changes against '{branch}'");
    (prompt, hint)
}

/// Build the review prompt and hint for a specific commit.
/// If `subject_opt` is provided, it will be included in the prompt; otherwise it is omitted.
pub fn review_commit_prompt(sha: &str, subject_opt: Option<&str>) -> (String, String) {
    let short = &sha[..min(7, sha.len())];
    let prompt = if let Some(subject) = subject_opt {
        format!(
            "Review the code changes introduced by commit {sha} (\"{subject}\"). Provide prioritized, actionable findings."
        )
    } else {
        format!(
            "Review the code changes introduced by commit {sha}. Provide prioritized, actionable findings."
        )
    };
    let hint = format!("commit {short}");
    (prompt, hint)
}

/// Build the review prompt and hint for custom free-form instructions.
pub fn review_custom_prompt(custom: &str) -> (String, String) {
    let prompt = custom.trim().to_string();
    let trimmed = prompt.clone();
    let hint = if trimmed.is_empty() {
        "custom review".to_string()
    } else {
        let s = trimmed.replace('\n', " ");
        let max = 80usize;
        if s.len() > max {
            format!("{}…", &s[..max])
        } else {
            s
        }
    };
    (prompt, hint)
}
