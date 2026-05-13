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
