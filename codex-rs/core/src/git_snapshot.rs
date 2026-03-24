use std::path::Path;
use std::process::Output;

use tempfile::tempdir;
use tokio::process::Command;
use tokio::time::Duration as TokioDuration;
use tokio::time::timeout;

use crate::git_info::get_git_repo_root;
use crate::git_info::get_head_commit_hash;

const GIT_SNAPSHOT_TIMEOUT: TokioDuration = TokioDuration::from_secs(30);
const SNAPSHOT_AUTHOR_DATE: &str = "1970-01-01T00:00:00Z";
const SNAPSHOT_AUTHOR_EMAIL: &str = "codex@openai.invalid";
const SNAPSHOT_AUTHOR_NAME: &str = "Codex";
const SNAPSHOT_MESSAGE: &str = "codex workspace snapshot";

pub async fn prepare_workspace_snapshot_commit(cwd: &Path) -> Option<String> {
    let repo_root = get_git_repo_root(cwd)?;
    let head = get_head_commit_hash(&repo_root).await?;
    let index_dir = tempdir().ok()?;
    let index_path = index_dir.path().join("workspace-snapshot.index");

    run_snapshot_command({
        let mut command = git_command(repo_root.as_path());
        command
            .arg("read-tree")
            .arg(&head)
            .env("GIT_INDEX_FILE", &index_path);
        command
    })
    .await?;

    run_snapshot_command({
        let mut command = git_command(repo_root.as_path());
        command
            .arg("add")
            .arg("-A")
            .env("GIT_INDEX_FILE", &index_path);
        command
    })
    .await?;

    let tree = String::from_utf8(
        run_snapshot_command({
            let mut command = git_command(repo_root.as_path());
            command.arg("write-tree").env("GIT_INDEX_FILE", &index_path);
            command
        })
        .await?
        .stdout,
    )
    .ok()?
    .trim()
    .to_string();

    let head_tree = String::from_utf8(
        run_snapshot_command({
            let mut command = git_command(repo_root.as_path());
            command.arg("rev-parse").arg(format!("{head}^{{tree}}"));
            command
        })
        .await?
        .stdout,
    )
    .ok()?
    .trim()
    .to_string();

    if tree == head_tree {
        return Some(head);
    }

    String::from_utf8(
        run_snapshot_command({
            let mut command = git_command(repo_root.as_path());
            command
                .arg("commit-tree")
                .arg(&tree)
                .arg("-p")
                .arg(&head)
                .arg("-m")
                .arg(SNAPSHOT_MESSAGE)
                .env("GIT_AUTHOR_DATE", SNAPSHOT_AUTHOR_DATE)
                .env("GIT_AUTHOR_EMAIL", SNAPSHOT_AUTHOR_EMAIL)
                .env("GIT_AUTHOR_NAME", SNAPSHOT_AUTHOR_NAME)
                .env("GIT_COMMITTER_DATE", SNAPSHOT_AUTHOR_DATE)
                .env("GIT_COMMITTER_EMAIL", SNAPSHOT_AUTHOR_EMAIL)
                .env("GIT_COMMITTER_NAME", SNAPSHOT_AUTHOR_NAME);
            command
        })
        .await?
        .stdout,
    )
    .ok()
    .map(|stdout| stdout.trim().to_string())
}

fn git_command(cwd: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(cwd);
    command
}

async fn run_snapshot_command(mut command: Command) -> Option<Output> {
    match timeout(GIT_SNAPSHOT_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => {
            if output.status.success() {
                Some(output)
            } else {
                tracing::warn!(
                    exit_code = output.status.code(),
                    stderr = %String::from_utf8_lossy(&output.stderr),
                    stdout = %String::from_utf8_lossy(&output.stdout),
                    "git workspace snapshot command failed",
                );
                None
            }
        }
        Ok(Err(err)) => {
            tracing::warn!(error = %err, "git workspace snapshot command errored");
            None
        }
        Err(_) => {
            tracing::warn!(
                timeout_sec = GIT_SNAPSHOT_TIMEOUT.as_secs(),
                "git workspace snapshot command timed out",
            );
            None
        }
    }
}

#[cfg(test)]
#[path = "git_snapshot_tests.rs"]
mod tests;
