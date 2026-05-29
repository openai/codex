//! Branch and pull-request metadata for TUI status-line items.
//!
//! This module owns the git and GitHub probes behind the TUI `git-branch`, `pull-request-number`,
//! and `branch-changes` status-line items. It deliberately talks only to a
//! `WorkspaceCommandExecutor`, not to `tokio::process::Command`, so the same lookup logic works
//! when the TUI is connected to either an embedded or remote app-server.
//!
//! All lookups are best-effort. A failed command, missing `git` or `gh`, unauthenticated GitHub
//! CLI, non-git directory, or ambiguous repository state should result in absent optional metadata
//! rather than a user-visible error. The status line can then render whichever pieces are available
//! without blocking the rest of the UI.

#[cfg(test)]
use std::collections::VecDeque;
use std::path::Path;

use serde::Deserialize;

use crate::workspace_command::WorkspaceCommand;
#[cfg(test)]
use crate::workspace_command::WorkspaceCommandError;
use crate::workspace_command::WorkspaceCommandExecutor;
use crate::workspace_command::WorkspaceCommandOutput;

/// Additions and deletions between `HEAD` and a branch comparison base.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitBranchDiffStats {
    /// Total added lines in committed changes on the current branch.
    pub(crate) additions: u64,
    /// Total deleted lines in committed changes on the current branch.
    pub(crate) deletions: u64,
}

/// Combined git metadata cached by the status line for one working directory.
///
/// A summary may contain only one of the fields when the other probe fails. Renderers should treat
/// missing fields as omitted optional UI rather than as a hard lookup failure.
#[derive(Clone, Debug, Default)]
pub(crate) struct StatusLineGitSummary {
    /// Open pull request associated with the current branch or HEAD commit.
    pub(crate) pull_request: Option<StatusLinePullRequest>,
    /// Additions and deletions between `HEAD` and the repository default branch merge base.
    pub(crate) branch_change_stats: Option<GitBranchDiffStats>,
}

/// Open GitHub pull request shown by the `pull-request-number` status-line item.
///
/// The URL is kept with the number so clickable renderers can open the same PR represented by the
/// compact label. Callers should only construct this for open PRs; closed or merged PRs are filtered
/// out by this module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StatusLinePullRequest {
    /// GitHub pull request number.
    pub(crate) number: u64,
    /// Browser URL for the pull request.
    pub(crate) url: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DefaultBranch {
    /// Git ref used for merge-base comparison.
    ///
    /// This may be a remote-tracking ref such as `refs/remotes/origin/main`, which avoids
    /// comparing against a stale or absent local `main` branch.
    merge_ref: String,
}

#[derive(Deserialize)]
struct GhPullRequestView {
    number: u64,
    url: String,
    state: String,
}

#[derive(Deserialize)]
struct GhPullRequestApiItem {
    number: u64,
    #[serde(rename = "html_url")]
    url: String,
    state: String,
}

#[derive(Deserialize)]
struct GhRepoView {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: Option<String>,
    parent: Option<GhRepoParent>,
}

#[derive(Deserialize)]
struct GhRepoParent {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
}

/// Returns the checked-out branch name for one status-line working directory.
///
/// Detached HEADs, non-git directories, and command failures return `None` so the renderer can
/// omit the branch item without surfacing a background lookup error.
pub(crate) async fn current_branch_name(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Option<String> {
    let output = run_git_command(runner, cwd, &["branch", "--show-current"])
        .await
        .ok()?;
    if !output.success() {
        return None;
    }

    Some(output.stdout.trim().to_string()).filter(|name| !name.is_empty())
}

/// Resolves PR and branch-change metadata for one status-line working directory.
///
/// The PR and diff-stat probes run concurrently because each is independent and both are optional.
/// The returned summary is suitable for caching by `cwd`; callers should discard it if the active
/// status-line cwd changes before the async lookup completes.
pub(crate) async fn status_line_git_summary(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> StatusLineGitSummary {
    let (pull_request, branch_change_stats) = tokio::join!(
        open_pull_request(runner, cwd),
        branch_diff_stats_to_default_branch(runner, cwd),
    );
    StatusLineGitSummary {
        pull_request,
        branch_change_stats,
    }
}

/// Counts committed line changes between `HEAD` and the repository default branch.
///
/// The comparison base is the merge base with a verified default-branch ref. Uncommitted working
/// tree edits are intentionally ignored because the status-line item summarizes the checked-out
/// branch, not the current dirty worktree.
async fn branch_diff_stats_to_default_branch(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Option<GitBranchDiffStats> {
    let git_dir = run_git_command(runner, cwd, &["rev-parse", "--git-dir"])
        .await
        .ok()?;
    if !git_dir.success() {
        return None;
    }

    let default_branch = get_default_branch(runner, cwd).await?;
    let merge_base = run_git_command(
        runner,
        cwd,
        &["merge-base", "HEAD", &default_branch.merge_ref],
    )
    .await
    .ok()?;
    if !merge_base.success() {
        return None;
    }
    let merge_base = merge_base.stdout.trim();
    if merge_base.is_empty() {
        return None;
    }

    let range = format!("{merge_base}..HEAD");
    let numstat = run_git_command(runner, cwd, &["diff", "--numstat", &range])
        .await
        .ok()?;
    if !numstat.success() {
        return None;
    }

    let mut additions = 0_u64;
    let mut deletions = 0_u64;
    for line in numstat.stdout.lines() {
        let mut columns = line.split('\t');
        additions += columns
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        deletions += columns
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
    }

    Some(GitBranchDiffStats {
        additions,
        deletions,
    })
}

/// Returns git remotes in the order used for default-branch discovery.
///
/// `origin` is prioritized because most repositories use it as the canonical upstream. Other
/// remotes are still tried so fork or enterprise layouts with a differently named upstream can
/// produce branch-change stats when their remote HEAD is configured.
async fn get_git_remotes(runner: &dyn WorkspaceCommandExecutor, cwd: &Path) -> Option<Vec<String>> {
    let output = run_git_command(runner, cwd, &["remote"]).await.ok()?;
    if !output.success() {
        return None;
    }

    let mut remotes: Vec<String> = output.stdout.lines().map(str::to_string).collect();
    if let Some(pos) = remotes.iter().position(|remote| remote == "origin") {
        let origin = remotes.remove(pos);
        remotes.insert(0, origin);
    }
    Some(remotes)
}

/// Resolves the default branch ref that should be used for branch-change comparisons.
///
/// The lookup prefers remote-tracking refs over local branches so feature-only clones and stale
/// local `main` branches do not inflate the status-line diff. When no remote default is available,
/// local `main` or `master` is used as a last resort.
async fn get_default_branch(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Option<DefaultBranch> {
    let remotes = get_git_remotes(runner, cwd).await.unwrap_or_default();
    for remote in remotes {
        if let Some(branch) =
            get_remote_default_branch_from_symbolic_ref(runner, cwd, &remote).await
        {
            return Some(branch);
        }

        if let Some(branch) = get_remote_default_branch_from_remote_show(runner, cwd, &remote).await
        {
            return Some(branch);
        }
    }

    get_default_branch_local(runner, cwd).await
}

/// Resolves a remote's symbolic HEAD into a concrete remote-tracking ref.
///
/// The returned ref is verified before use. Without that check, a symbolic `origin/HEAD` left over
/// from an old fetch could point at a ref that no longer exists, causing the later merge-base probe
/// to fail in a less obvious place.
async fn get_remote_default_branch_from_symbolic_ref(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    remote: &str,
) -> Option<DefaultBranch> {
    let remote_head = format!("refs/remotes/{remote}/HEAD");
    let output = run_git_command(runner, cwd, &["symbolic-ref", "--quiet", &remote_head])
        .await
        .ok()?;
    if !output.success() {
        return None;
    }

    let trimmed = output.stdout.trim();
    let remote_ref_prefix = format!("refs/remotes/{remote}/");
    trimmed.strip_prefix(&remote_ref_prefix)?;
    if !git_ref_exists(runner, cwd, trimmed).await {
        return None;
    }

    Some(DefaultBranch {
        merge_ref: trimmed.to_string(),
    })
}

/// Parses `git remote show` output to discover a remote's default branch ref.
///
/// This is a fallback for repositories where `refs/remotes/<remote>/HEAD` is not configured but
/// `git remote show` can still report the upstream HEAD branch. The concrete remote-tracking ref
/// must already exist locally before it is accepted.
async fn get_remote_default_branch_from_remote_show(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    remote: &str,
) -> Option<DefaultBranch> {
    let output = run_git_command(runner, cwd, &["remote", "show", remote])
        .await
        .ok()?;
    if !output.success() {
        return None;
    }

    for line in output.stdout.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("HEAD branch:") else {
            continue;
        };
        let name = rest.trim();
        let remote_ref = format!("refs/remotes/{remote}/{name}");
        if !name.is_empty() && git_ref_exists(runner, cwd, &remote_ref).await {
            return Some(DefaultBranch {
                merge_ref: remote_ref,
            });
        }
    }

    None
}

/// Falls back to local `main` or `master` when no remote default branch can be found.
async fn get_default_branch_local(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Option<DefaultBranch> {
    for candidate in ["main", "master"] {
        let local_ref = format!("refs/heads/{candidate}");
        if git_ref_exists(runner, cwd, &local_ref).await {
            return Some(DefaultBranch {
                merge_ref: local_ref,
            });
        }
    }

    None
}

/// Checks whether a git ref exists in the status-line working directory.
async fn git_ref_exists(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    reference: &str,
) -> bool {
    run_git_command(
        runner,
        cwd,
        &["rev-parse", "--verify", "--quiet", reference],
    )
    .await
    .is_ok_and(|output| output.success())
}

/// Resolves the open PR associated with the current checkout.
///
/// Branch-based lookup is attempted first because it is cheap and mirrors `gh pr view`. Commit-based
/// lookup is used as a fallback so fork workflows can still find a PR opened against the upstream
/// repository even when `gh` infers the fork from the current checkout.
async fn open_pull_request(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Option<StatusLinePullRequest> {
    if let Some(pull_request) = open_pull_request_for_current_branch(runner, cwd).await {
        return Some(pull_request);
    }

    open_pull_request_for_head_commit(runner, cwd).await
}

/// Uses GitHub CLI's current-branch PR lookup.
async fn open_pull_request_for_current_branch(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Option<StatusLinePullRequest> {
    let output = run_gh_command(runner, cwd, &["pr", "view", "--json", "number,url,state"])
        .await
        .ok()?;
    if !output.success() {
        return None;
    }
    pull_request_from_view_output(&output.stdout)
}

/// Looks up open PRs for `HEAD` across the upstream/fork repository search order.
async fn open_pull_request_for_head_commit(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Option<StatusLinePullRequest> {
    let head_sha = current_head_sha(runner, cwd).await?;
    for repo in gh_repo_search_order(runner, cwd).await? {
        let endpoint = format!("repos/{repo}/commits/{head_sha}/pulls");
        let output = run_gh_command(
            runner,
            cwd,
            &[
                "api",
                "-H",
                "Accept: application/vnd.github+json",
                &endpoint,
            ],
        )
        .await
        .ok()?;
        if output.success()
            && let Some(pull_request) = pull_request_from_api_output(&output.stdout)
        {
            return Some(pull_request);
        }
    }

    None
}

/// Returns the current `HEAD` SHA for commit-based PR lookup.
async fn current_head_sha(runner: &dyn WorkspaceCommandExecutor, cwd: &Path) -> Option<String> {
    let output = run_git_command(runner, cwd, &["rev-parse", "HEAD"])
        .await
        .ok()?;
    if !output.success() {
        return None;
    }

    Some(output.stdout.trim().to_string()).filter(|sha| !sha.is_empty())
}

/// Returns repositories to query for commit-associated PRs, with parent before fork.
async fn gh_repo_search_order(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
) -> Option<Vec<String>> {
    let output = run_gh_command(
        runner,
        cwd,
        &["repo", "view", "--json", "nameWithOwner,parent"],
    )
    .await
    .ok()?;
    if !output.success() {
        return None;
    }

    repo_search_order_from_output(&output.stdout)
}

/// Parses `gh pr view --json number,url,state` output for an open PR.
fn pull_request_from_view_output(stdout: &str) -> Option<StatusLinePullRequest> {
    let pull_request = serde_json::from_str::<GhPullRequestView>(stdout).ok()?;
    pull_request
        .state
        .eq_ignore_ascii_case("open")
        .then_some(StatusLinePullRequest {
            number: pull_request.number,
            url: pull_request.url,
        })
}

/// Parses the GitHub REST commit-to-PR response and returns the first open PR.
fn pull_request_from_api_output(stdout: &str) -> Option<StatusLinePullRequest> {
    serde_json::from_str::<Vec<GhPullRequestApiItem>>(stdout)
        .ok()?
        .into_iter()
        .find(|pull_request| pull_request.state.eq_ignore_ascii_case("open"))
        .map(|pull_request| StatusLinePullRequest {
            number: pull_request.number,
            url: pull_request.url,
        })
}

/// Parses `gh repo view` output into the repository search order for fallback PR lookup.
///
/// Parent-first ordering matches upstream PR workflows: a branch may be checked out from a fork
/// while the open PR lives on the parent repository.
fn repo_search_order_from_output(stdout: &str) -> Option<Vec<String>> {
    let repo = serde_json::from_str::<GhRepoView>(stdout).ok()?;
    let mut repos = Vec::new();
    if let Some(parent) = repo.parent {
        repos.push(parent.name_with_owner);
    }
    if let Some(name_with_owner) = repo.name_with_owner
        && !repos.iter().any(|repo| repo == &name_with_owner)
    {
        repos.push(name_with_owner);
    }
    if repos.is_empty() {
        return None;
    }

    Some(repos)
}

/// Runs a git command through the workspace-command abstraction.
async fn run_git_command(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    args: &[&str],
) -> Result<WorkspaceCommandOutput, crate::workspace_command::WorkspaceCommandError> {
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push("git".to_string());
    argv.extend(args.iter().map(|arg| (*arg).to_string()));
    runner
        .run(
            WorkspaceCommand::new(argv)
                .cwd(cwd.to_path_buf())
                .env("GIT_OPTIONAL_LOCKS", "0"),
        )
        .await
}

/// Runs a GitHub CLI command through the workspace-command abstraction.
///
/// Prompting is disabled because status-line probes are background UI work. A command that needs
/// authentication or user input should fail and leave the optional PR item hidden.
async fn run_gh_command(
    runner: &dyn WorkspaceCommandExecutor,
    cwd: &Path,
    args: &[&str],
) -> Result<WorkspaceCommandOutput, crate::workspace_command::WorkspaceCommandError> {
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push("gh".to_string());
    argv.extend(args.iter().map(|arg| (*arg).to_string()));
    runner
        .run(
            WorkspaceCommand::new(argv)
                .cwd(cwd.to_path_buf())
                .env("GH_PROMPT_DISABLED", "1")
                .env("GIT_TERMINAL_PROMPT", "0"),
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_command::WorkspaceCommand;
    use pretty_assertions::assert_eq;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    #[tokio::test]
    async fn branch_diff_stats_prefers_remote_default_ref_over_stale_local_branch() {
        let runner = FakeRunner::new(vec![
            response(
                &["git", "rev-parse", "--git-dir"],
                /*exit_code*/ 0,
                ".git\n",
            ),
            response(&["git", "remote"], /*exit_code*/ 0, "origin\n"),
            response(
                &["git", "symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"],
                /*exit_code*/ 0,
                "refs/remotes/origin/main\n",
            ),
            response(
                &[
                    "git",
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    "refs/remotes/origin/main",
                ],
                /*exit_code*/ 0,
                "remote-main-sha\n",
            ),
            response(
                &["git", "merge-base", "HEAD", "refs/remotes/origin/main"],
                /*exit_code*/ 0,
                "base-sha\n",
            ),
            response(
                &["git", "diff", "--numstat", "base-sha..HEAD"],
                /*exit_code*/ 0,
                "1\t0\tfile\n",
            ),
        ]);

        let stats = branch_diff_stats_to_default_branch(&runner, Path::new("/repo"))
            .await
            .expect("branch diff stats");

        assert_eq!(
            stats,
            GitBranchDiffStats {
                additions: 1,
                deletions: 0,
            }
        );
        assert!(runner.saw(&["git", "merge-base", "HEAD", "refs/remotes/origin/main"]));
    }

    #[tokio::test]
    async fn open_pull_request_uses_current_branch_view_first() {
        let runner = FakeRunner::new(vec![response(
            &["gh", "pr", "view", "--json", "number,url,state"],
            /*exit_code*/ 0,
            r#"{"number":20252,"url":"https://github.com/openai/codex/pull/20252","state":"OPEN"}"#,
        )]);

        let pull_request = open_pull_request(&runner, Path::new("/repo"))
            .await
            .expect("pull request");

        assert_eq!(
            pull_request,
            StatusLinePullRequest {
                number: 20_252,
                url: "https://github.com/openai/codex/pull/20252".to_string(),
            }
        );
        assert!(!runner.saw(&["git", "rev-parse", "HEAD"]));
    }

    #[tokio::test]
    async fn open_pull_request_falls_back_to_parent_repo_commit_lookup() {
        let runner = FakeRunner::new(vec![
            response(
                &["gh", "pr", "view", "--json", "number,url,state"],
                /*exit_code*/ 1,
                "",
            ),
            response(
                &["git", "rev-parse", "HEAD"],
                /*exit_code*/ 0,
                "head-sha\n",
            ),
            response(
                &["gh", "repo", "view", "--json", "nameWithOwner,parent"],
                /*exit_code*/ 0,
                r#"{"nameWithOwner":"fcoury/codex","parent":{"nameWithOwner":"openai/codex"}}"#,
            ),
            response(
                &[
                    "gh",
                    "api",
                    "-H",
                    "Accept: application/vnd.github+json",
                    "repos/openai/codex/commits/head-sha/pulls",
                ],
                /*exit_code*/ 0,
                r#"[{"number":20252,"html_url":"https://github.com/openai/codex/pull/20252","state":"open"}]"#,
            ),
        ]);

        let pull_request = open_pull_request(&runner, Path::new("/repo"))
            .await
            .expect("pull request");

        assert_eq!(
            pull_request,
            StatusLinePullRequest {
                number: 20_252,
                url: "https://github.com/openai/codex/pull/20252".to_string(),
            }
        );
        assert!(runner.saw(&[
            "gh",
            "api",
            "-H",
            "Accept: application/vnd.github+json",
            "repos/openai/codex/commits/head-sha/pulls",
        ]));
    }

    #[test]
    fn status_line_pr_view_parser_requires_open_pr() {
        assert_eq!(
            pull_request_from_view_output(
                r#"{"number":20252,"url":"https://github.com/openai/codex/pull/20252","state":"OPEN"}"#
            ),
            Some(StatusLinePullRequest {
                number: 20_252,
                url: "https://github.com/openai/codex/pull/20252".to_string(),
            })
        );

        assert_eq!(
            pull_request_from_view_output(
                r#"{"number":20252,"url":"https://github.com/openai/codex/pull/20252","state":"MERGED"}"#
            ),
            None
        );
    }

    #[test]
    fn status_line_pr_fallback_searches_parent_repo_first() {
        assert_eq!(
            repo_search_order_from_output(
                r#"{"nameWithOwner":"fcoury/codex","parent":{"nameWithOwner":"openai/codex"}}"#
            ),
            Some(vec!["openai/codex".to_string(), "fcoury/codex".to_string()])
        );
    }

    fn response(argv: &[&str], exit_code: i32, stdout: &str) -> FakeResponse {
        FakeResponse {
            argv: argv.iter().map(|arg| (*arg).to_string()).collect(),
            output: WorkspaceCommandOutput {
                exit_code,
                stdout: stdout.to_string(),
                stderr: String::new(),
            },
        }
    }

    struct FakeResponse {
        argv: Vec<String>,
        output: WorkspaceCommandOutput,
    }

    struct FakeRunner {
        responses: Mutex<VecDeque<FakeResponse>>,
        seen: Mutex<Vec<Vec<String>>>,
    }

    impl FakeRunner {
        fn new(responses: Vec<FakeResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn saw(&self, argv: &[&str]) -> bool {
            let argv: Vec<String> = argv.iter().map(|arg| (*arg).to_string()).collect();
            self.seen
                .lock()
                .expect("seen lock")
                .iter()
                .any(|seen| seen == &argv)
        }
    }

    impl WorkspaceCommandExecutor for FakeRunner {
        fn run(
            &self,
            command: WorkspaceCommand,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<WorkspaceCommandOutput, WorkspaceCommandError>>
                    + Send
                    + '_,
            >,
        > {
            self.seen
                .lock()
                .expect("seen lock")
                .push(command.argv.clone());
            Box::pin(async move {
                let mut responses = self.responses.lock().expect("responses lock");
                let index = responses
                    .iter()
                    .position(|response| response.argv == command.argv)
                    .unwrap_or_else(|| panic!("missing fake response for {:?}", command.argv));
                let response = responses.remove(index).expect("fake response");
                Ok(response.output)
            })
        }
    }
}
