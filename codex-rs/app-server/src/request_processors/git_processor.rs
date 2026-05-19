use super::*;

#[derive(Clone)]
pub(crate) struct GitRequestProcessor {
    runtime_capabilities: Arc<RuntimeCapabilities>,
}

impl GitRequestProcessor {
    pub(crate) fn new(runtime_capabilities: Arc<RuntimeCapabilities>) -> Self {
        Self {
            runtime_capabilities,
        }
    }

    pub(crate) async fn git_diff_to_remote(
        &self,
        params: GitDiffToRemoteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.runtime_capabilities
            .require_local_environment("git diff to remote")
            .map_err(|err| method_not_found(err.to_string()))?;
        self.git_diff_to_origin(params.cwd)
            .await
            .map(|response| Some(response.into()))
    }

    async fn git_diff_to_origin(
        &self,
        cwd: PathBuf,
    ) -> Result<GitDiffToRemoteResponse, JSONRPCErrorError> {
        git_diff_to_remote(&cwd)
            .await
            .map(|value| GitDiffToRemoteResponse {
                sha: value.sha,
                diff: value.diff,
            })
            .ok_or_else(|| {
                invalid_request(format!(
                    "failed to compute git diff to remote for cwd: {cwd:?}"
                ))
            })
    }
}
