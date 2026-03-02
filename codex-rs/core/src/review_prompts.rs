use codex_git::merge_base_with_head;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedReviewRequest {
    pub target: ReviewTarget,
    pub prompt: String,
    pub user_facing_hint: String,
}

const UNCOMMITTED_PROMPT: &str = "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.";

const BASE_BRANCH_PROMPT_BACKUP: &str = "Review the code changes against the base branch '{branch}'. Start by finding the merge diff between the current branch and {branch}'s upstream e.g. (`git merge-base HEAD \"$(git rev-parse --abbrev-ref \"{branch}@{upstream}\")\"`), then run `git diff` against that SHA to see what changes we would merge into the {branch} branch. Provide prioritized, actionable findings.";
const BASE_BRANCH_PROMPT: &str = "Review the code changes against the base branch '{baseBranch}'. The merge base commit for this comparison is {mergeBaseSha}. Run `git diff {mergeBaseSha}` to inspect the changes relative to {baseBranch}. Provide prioritized, actionable findings.";

const COMMIT_PROMPT_WITH_TITLE: &str = "Review the code changes introduced by commit {sha} (\"{title}\"). Provide prioritized, actionable findings.";
const COMMIT_PROMPT: &str =
    "Review the code changes introduced by commit {sha}. Provide prioritized, actionable findings.";
const COMMIT_FOLLOW_UP_PROMPT_WITH_TITLE: &str = "Review the cumulative code changes from {sha}^ through HEAD, rooted at commit {sha} (\"{title}\"). Run `git diff {sha}^ HEAD` to inspect the updated stack. Provide prioritized, actionable findings.";
const COMMIT_FOLLOW_UP_PROMPT: &str = "Review the cumulative code changes from {sha}^ through HEAD, rooted at commit {sha}. Run `git diff {sha}^ HEAD` to inspect the updated stack. Provide prioritized, actionable findings.";

pub fn resolve_review_request(
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

pub fn review_prompt(target: &ReviewTarget, cwd: &Path) -> anyhow::Result<String> {
    match target {
        ReviewTarget::UncommittedChanges => Ok(UNCOMMITTED_PROMPT.to_string()),
        ReviewTarget::BaseBranch { branch } => {
            if let Some(commit) = merge_base_with_head(cwd, branch)? {
                Ok(BASE_BRANCH_PROMPT
                    .replace("{baseBranch}", branch)
                    .replace("{mergeBaseSha}", &commit))
            } else {
                Ok(BASE_BRANCH_PROMPT_BACKUP.replace("{branch}", branch))
            }
        }
        ReviewTarget::Commit { sha, title } => {
            if let Some(title) = title {
                Ok(COMMIT_PROMPT_WITH_TITLE
                    .replace("{sha}", sha)
                    .replace("{title}", title))
            } else {
                Ok(COMMIT_PROMPT.replace("{sha}", sha))
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

pub fn review_prompt_with_additional_instructions(
    target: &ReviewTarget,
    cwd: &Path,
    additional_instructions: Option<&str>,
) -> anyhow::Result<String> {
    let prompt = review_prompt(target, cwd)?;
    Ok(append_additional_review_instructions(
        prompt,
        additional_instructions,
    ))
}

pub fn commit_follow_up_review_prompt(
    sha: &str,
    title: Option<&str>,
    additional_instructions: Option<&str>,
) -> String {
    let prompt = if let Some(title) = title {
        COMMIT_FOLLOW_UP_PROMPT_WITH_TITLE
            .replace("{sha}", sha)
            .replace("{title}", title)
    } else {
        COMMIT_FOLLOW_UP_PROMPT.replace("{sha}", sha)
    };
    append_additional_review_instructions(prompt, additional_instructions)
}

fn append_additional_review_instructions(
    prompt: String,
    additional_instructions: Option<&str>,
) -> String {
    let Some(additional_instructions) = additional_instructions.map(str::trim) else {
        return prompt;
    };
    if additional_instructions.is_empty() {
        return prompt;
    }
    format!("{prompt}\n\nAdditional review instructions:\n{additional_instructions}")
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
    fn review_prompt_with_additional_instructions_appends_header() {
        let cwd = Path::new("/tmp");
        let prompt = review_prompt_with_additional_instructions(
            &ReviewTarget::UncommittedChanges,
            cwd,
            Some("Focus on migration safety."),
        )
        .expect("prompt");
        assert_eq!(
            prompt,
            "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.\n\nAdditional review instructions:\nFocus on migration safety."
        );
    }

    #[test]
    fn commit_follow_up_review_prompt_references_cumulative_stack() {
        let prompt =
            commit_follow_up_review_prompt("abc1234", Some("Add review loop"), Some("Find bugs."));
        assert_eq!(
            prompt,
            "Review the cumulative code changes from abc1234^ through HEAD, rooted at commit abc1234 (\"Add review loop\"). Run `git diff abc1234^ HEAD` to inspect the updated stack. Provide prioritized, actionable findings.\n\nAdditional review instructions:\nFind bugs."
        );
    }

    #[test]
    fn commit_follow_up_review_prompt_omits_empty_additional_instructions() {
        let prompt = commit_follow_up_review_prompt("abc1234", None, Some("   "));
        assert_eq!(
            prompt,
            "Review the cumulative code changes from abc1234^ through HEAD, rooted at commit abc1234. Run `git diff abc1234^ HEAD` to inspect the updated stack. Provide prioritized, actionable findings."
        );
    }
}
