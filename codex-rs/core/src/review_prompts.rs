use codex_git::branch_upstream;
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
const BASE_BRANCH_PROMPT_WITH_UPSTREAM: &str = "Review the code changes against the base branch '{branch}', whose configured upstream is '{upstream}'. If tool permissions allow it, refresh that upstream ref first (for example with `git fetch`) so the comparison uses the latest remote state. Then compare '{branch}' and '{upstream}'. If '{upstream}' is ahead of '{branch}' at all, including diverged histories where both refs have unique commits, use '{upstream}' as the base ref for this review; otherwise use '{branch}'. Find the merge diff between the current branch and that chosen base ref, and run `git diff` against the merge-base SHA to inspect what would merge into {branch}. Provide prioritized, actionable findings.";
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
            if let Some(upstream) = branch_upstream(cwd, branch)? {
                Ok(BASE_BRANCH_PROMPT_WITH_UPSTREAM
                    .replace("{branch}", branch)
                    .replace("{upstream}", &upstream))
            } else if let Some(commit) = merge_base_with_head(cwd, branch)? {
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
            validate_findings: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::process::Command;
    use tempfile::tempdir;

    fn run_git_in(repo_path: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .status()
            .expect("git command");
        assert!(status.success(), "git command failed: {args:?}");
    }

    fn commit(repo_path: &Path, message: &str) {
        run_git_in(
            repo_path,
            &[
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                message,
            ],
        );
    }

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

    #[test]
    fn review_prompt_uses_newer_of_local_or_upstream_when_branch_tracks_remote() {
        let temp = tempdir().expect("temp dir");
        let repo = temp.path().join("repo");
        let remote = temp.path().join("remote.git");
        std::fs::create_dir_all(&repo).expect("repo dir");
        std::fs::create_dir_all(&remote).expect("remote dir");

        run_git_in(&remote, &["init", "--bare"]);
        run_git_in(&repo, &["init", "--initial-branch=main"]);
        run_git_in(&repo, &["config", "core.autocrlf", "false"]);
        std::fs::write(repo.join("base.txt"), "base\n").expect("write base");
        run_git_in(&repo, &["add", "base.txt"]);
        commit(&repo, "base commit");
        run_git_in(
            &repo,
            &[
                "remote",
                "add",
                "origin",
                remote.to_str().expect("remote path"),
            ],
        );
        run_git_in(&repo, &["push", "-u", "origin", "main"]);

        let prompt = review_prompt(
            &ReviewTarget::BaseBranch {
                branch: "main".to_string(),
            },
            &repo,
        )
        .expect("prompt");

        assert_eq!(
            prompt,
            "Review the code changes against the base branch 'main', whose configured upstream is 'origin/main'. If tool permissions allow it, refresh that upstream ref first (for example with `git fetch`) so the comparison uses the latest remote state. Then compare 'main' and 'origin/main'. If 'origin/main' is ahead of 'main' at all, including diverged histories where both refs have unique commits, use 'origin/main' as the base ref for this review; otherwise use 'main'. Find the merge diff between the current branch and that chosen base ref, and run `git diff` against the merge-base SHA to inspect what would merge into main. Provide prioritized, actionable findings."
        );
    }
}
