use super::*;

#[derive(Clone)]
pub(crate) struct GitRequestProcessor;

impl GitRequestProcessor {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn git_diff_to_remote(
        &self,
        params: GitDiffToRemoteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.git_diff_to_origin(params.cwd)
            .await
            .map(|response| Some(response.into()))
    }

    async fn git_diff_to_origin(
        &self,
        cwd: PathBuf,
    ) -> Result<GitDiffToRemoteResponse, JSONRPCErrorError> {
        try_git_diff_to_remote(&cwd)
            .await
            .map(|value| GitDiffToRemoteResponse {
                sha: value.sha,
                diff: value.diff,
            })
            .map_err(|reason| {
                let mut error = invalid_request(format!(
                    "failed to compute git diff to remote for cwd {cwd:?}: {reason}"
                ));
                error.data = serde_json::to_value(&reason).ok();
                error
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(cwd: &std::path::Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("run Git");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn selected_filter_failure_has_structured_json_rpc_data() {
        let repo = tempfile::tempdir().expect("repository");
        let root = repo.path();
        git(root, &["init"]);
        git(root, &["config", "user.email", "codex@example.com"]);
        git(root, &["config", "user.name", "Codex"]);
        std::fs::write(root.join(".gitattributes"), "file.txt filter=blocked\n")
            .expect("attributes");
        std::fs::write(root.join("file.txt"), "contents\n").expect("file");
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "fixture"]);
        git(
            root,
            &[
                "config",
                "filter.blocked.clean",
                "git config codex.filterran true && git hash-object --stdin",
            ],
        );

        let error = GitRequestProcessor::new()
            .git_diff_to_origin(root.to_path_buf())
            .await
            .expect_err("selected filter must block diff");
        assert_eq!(
            error.data,
            Some(serde_json::json!({
                "reason": "selectedExecutableFilter",
                "driver": "blocked",
                "path": "file.txt",
            }))
        );
        let marker = std::process::Command::new("git")
            .args(["config", "--get", "codex.filterran"])
            .current_dir(root)
            .status()
            .expect("read marker");
        assert!(!marker.success(), "selected filter must not run");
    }
}
