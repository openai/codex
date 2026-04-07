use codex_git_utils::merge_base_with_head;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget;
use codex_utils_template::Template;
use std::path::Path;
use std::sync::LazyLock;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedReviewRequest {
    pub(crate) target: ReviewTarget,
    pub(crate) prompt: String,
    pub(crate) user_facing_hint: String,
}

const UNCOMMITTED_PROMPT: &str = "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.";

const BASE_BRANCH_PROMPT_BACKUP: &str = "Review the code changes against the base branch '{{branch}}'. Start by finding the merge diff between the current branch and {{branch}}'s upstream e.g. (`git merge-base HEAD \"$(git rev-parse --abbrev-ref \"{{branch}}@{upstream}\")\"`), then run `git diff` against that SHA to see what changes we would merge into the {{branch}} branch. Provide prioritized, actionable findings.";
const BASE_BRANCH_PROMPT: &str = "Review the code changes against the base branch '{{base_branch}}'. The merge base commit for this comparison is {{merge_base_sha}}. Run `git diff {{merge_base_sha}}` to inspect the changes relative to {{base_branch}}. Provide prioritized, actionable findings.";
static BASE_BRANCH_PROMPT_BACKUP_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(BASE_BRANCH_PROMPT_BACKUP)
        .unwrap_or_else(|err| panic!("base branch backup review prompt must parse: {err}"))
});
static BASE_BRANCH_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(BASE_BRANCH_PROMPT)
        .unwrap_or_else(|err| panic!("base branch review prompt must parse: {err}"))
});

const COMMIT_PROMPT_WITH_TITLE: &str = "Review the code changes introduced by commit {{sha}} (\"{{title}}\"). Provide prioritized, actionable findings.";
const COMMIT_PROMPT: &str = "Review the code changes introduced by commit {{sha}}. Provide prioritized, actionable findings.";
static COMMIT_PROMPT_WITH_TITLE_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(COMMIT_PROMPT_WITH_TITLE)
        .unwrap_or_else(|err| panic!("commit review prompt with title must parse: {err}"))
});
static COMMIT_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(COMMIT_PROMPT)
        .unwrap_or_else(|err| panic!("commit review prompt must parse: {err}"))
});

pub(crate) fn resolve_review_request(
    request: ReviewRequest,
    cwd: &Path,
) -> anyhow::Result<ResolvedReviewRequest> {
    let target = request.target;
    let prompt = review_prompt(&target, cwd)?;
    let user_facing_hint = request
        .user_facing_hint
        .unwrap_or_else(|| user_facing_hint(&target));

    Ok(ResolvedReviewRequest {
        target,
        prompt,
        user_facing_hint,
    })
}

pub(crate) fn review_prompt(target: &ReviewTarget, cwd: &Path) -> anyhow::Result<String> {
    match target {
        ReviewTarget::UncommittedChanges => Ok(UNCOMMITTED_PROMPT.to_string()),
        ReviewTarget::BaseBranch { branch } => {
            if let Some(commit) = merge_base_with_head(cwd, branch)? {
                Ok(render_review_prompt(
                    &BASE_BRANCH_PROMPT_TEMPLATE,
                    [
                        ("base_branch", branch.as_str()),
                        ("merge_base_sha", commit.as_str()),
                    ],
                ))
            } else {
                Ok(render_review_prompt(
                    &BASE_BRANCH_PROMPT_BACKUP_TEMPLATE,
                    [("branch", branch.as_str())],
                ))
            }
        }
        ReviewTarget::Commit { sha, title } => {
            if let Some(title) = title {
                Ok(render_review_prompt(
                    &COMMIT_PROMPT_WITH_TITLE_TEMPLATE,
                    [("sha", sha.as_str()), ("title", title.as_str())],
                ))
            } else {
                Ok(render_review_prompt(
                    &COMMIT_PROMPT_TEMPLATE,
                    [("sha", sha.as_str())],
                ))
            }
        }
        ReviewTarget::Custom { instructions } => {
            let prompt = instructions.trim();
            if prompt.is_empty() {
                anyhow::bail!("Review prompt cannot be empty");
            }
            Ok(prompt.to_string())
        }
    }
}

fn render_review_prompt<'a, const N: usize>(
    template: &Template,
    variables: [(&'a str, &'a str); N],
) -> String {
    template
        .render(variables)
        .unwrap_or_else(|err| panic!("review prompt template must render: {err}"))
}

pub fn user_facing_hint(target: &ReviewTarget) -> String {
    match target {
        ReviewTarget::UncommittedChanges => "current changes".to_string(),
        ReviewTarget::BaseBranch { branch } => format!("changes against '{branch}'"),
        ReviewTarget::Commit { sha, title } => {
            let short_sha: String = sha.chars().take(7).collect();
            if let Some(title) = title {
                format!("commit {short_sha}: {title}")
            } else {
                format!("commit {short_sha}")
            }
        }
        ReviewTarget::Custom { instructions } => instructions.trim().to_string(),
    }
}

impl From<ResolvedReviewRequest> for ReviewRequest {
    fn from(resolved: ResolvedReviewRequest) -> Self {
        ReviewRequest {
            target: resolved.target,
            user_facing_hint: Some(resolved.user_facing_hint),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn review_prompt_template_renders_base_branch_backup_variant() {
        assert_eq!(
            render_review_prompt(&BASE_BRANCH_PROMPT_BACKUP_TEMPLATE, [("branch", "main")]),
            "Review the code changes against the base branch 'main'. Start by finding the merge diff between the current branch and main's upstream e.g. (`git merge-base HEAD \"$(git rev-parse --abbrev-ref \"main@{upstream}\")\"`), then run `git diff` against that SHA to see what changes we would merge into the main branch. Provide prioritized, actionable findings."
        );
    }

    #[test]
    fn review_prompt_template_renders_base_branch_variant() {
        assert_eq!(
            render_review_prompt(
                &BASE_BRANCH_PROMPT_TEMPLATE,
                [("base_branch", "main"), ("merge_base_sha", "abc123")]
            ),
            "Review the code changes against the base branch 'main'. The merge base commit for this comparison is abc123. Run `git diff abc123` to inspect the changes relative to main. Provide prioritized, actionable findings."
        );
    }

    #[test]
    fn review_prompt_template_renders_commit_variant() {
        assert_eq!(
            review_prompt(
                &ReviewTarget::Commit {
                    sha: "deadbeef".to_string(),
                    title: None,
                },
                Path::new("."),
            )
            .expect("commit prompt should render"),
            "Review the code changes introduced by commit deadbeef. Provide prioritized, actionable findings."
        );
    }

    #[test]
    fn review_prompt_template_renders_commit_variant_with_title() {
        assert_eq!(
            review_prompt(
                &ReviewTarget::Commit {
                    sha: "deadbeef".to_string(),
                    title: Some("Fix bug".to_string()),
                },
                Path::new("."),
            )
            .expect("commit prompt should render"),
            "Review the code changes introduced by commit deadbeef (\"Fix bug\"). Provide prioritized, actionable findings."
        );
    }
}
