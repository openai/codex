use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::WeakShared;

use crate::info::detect_local_fsmonitor_override;
use crate::info::run_git_command_with_timeout_from;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GitStatusKey {
    git: PathBuf,
    repo_root: PathBuf,
}

type GitStatusFuture = BoxFuture<'static, Option<bool>>;

fn git_status_runs() -> &'static Mutex<HashMap<GitStatusKey, WeakShared<GitStatusFuture>>> {
    static RUNS: OnceLock<Mutex<HashMap<GitStatusKey, WeakShared<GitStatusFuture>>>> =
        OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn get_has_changes_in_repo(cwd: &Path, repo_root: &Path) -> Option<bool> {
    let git = PathBuf::from("git");
    let cwd = cwd.to_path_buf();
    let key = git_status_key(git.clone(), repo_root).await;
    share_git_status_run(key, move || async move {
        let fsmonitor = detect_local_fsmonitor_override(&git, &cwd).await;
        let output =
            run_git_command_with_timeout_from(&git, &["status", "--porcelain"], &cwd, fsmonitor)
                .await?;
        output.status.success().then_some(!output.stdout.is_empty())
    })
    .await
}

async fn git_status_key(git: PathBuf, repo_root: &Path) -> GitStatusKey {
    let repo_root = tokio::fs::canonicalize(repo_root)
        .await
        .unwrap_or_else(|_| repo_root.to_path_buf());
    GitStatusKey { git, repo_root }
}

async fn share_git_status_run<F, Fut>(key: GitStatusKey, run: F) -> Option<bool>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Option<bool>> + Send + 'static,
{
    let result = {
        let mut runs = git_status_runs()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(result) = runs
            .get(&key)
            .and_then(WeakShared::upgrade)
            .filter(|result| result.peek().is_none())
        {
            result
        } else {
            runs.retain(|_, result| {
                result
                    .upgrade()
                    .is_some_and(|result| result.peek().is_none())
            });
            let result = run().boxed().shared();
            if let Some(weak_result) = result.downgrade() {
                runs.insert(key, weak_result);
            }
            result
        }
    };
    result.await
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
