use super::*;
use codex_exec_server::RemoteEnvironmentOptions;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct EnvironmentRequestProcessor {
    environment_manager: Arc<EnvironmentManager>,
}

impl EnvironmentRequestProcessor {
    pub(crate) fn new(environment_manager: Arc<EnvironmentManager>) -> Self {
        Self {
            environment_manager,
        }
    }

    pub(crate) async fn environment_add(
        &self,
        params: EnvironmentAddParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let options = RemoteEnvironmentOptions {
            exec_server_url: params.exec_server_url,
            connect_timeout: params.connect_timeout_ms.map(Duration::from_millis),
            http_headers: params
                .auth_bearer_token
                .into_iter()
                .map(|token| {
                    (
                        "Authorization".to_string(),
                        format!("Bearer {}", token.into_inner()),
                    )
                })
                .collect(),
        };
        self.environment_manager
            .upsert_environment_with_options(params.environment_id, options)
            .map_err(|err| invalid_request(err.to_string()))?;
        Ok(Some(EnvironmentAddResponse {}.into()))
    }

    pub(crate) async fn environment_info(
        &self,
        params: EnvironmentInfoParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let environment_id = params.environment_id;
        let environment = self
            .environment_manager
            .get_environment(&environment_id)
            .ok_or_else(|| invalid_request(format!("unknown environment id `{environment_id}`")))?;
        let info = environment.force_info().await.map_err(|err| {
            internal_error(format!(
                "failed to get info for environment `{environment_id}`: {err}"
            ))
        })?;
        Ok(Some(
            EnvironmentInfoResponse {
                shell: EnvironmentShellInfo {
                    name: info.shell.name,
                    path: info.shell.path,
                },
                cwd: info.cwd,
            }
            .into(),
        ))
    }

    pub(crate) async fn environment_status(
        &self,
        params: EnvironmentStatusParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let environment_id = params.environment_id;
        let (status, error) = match self
            .environment_manager
            .get_environment_status(&environment_id)
            .await
        {
            Some(EnvironmentObservedStatus::Ready) => (EnvironmentStatusKind::Ready, None),
            Some(EnvironmentObservedStatus::Pending) => (EnvironmentStatusKind::Pending, None),
            Some(EnvironmentObservedStatus::Disconnected { error }) => {
                (EnvironmentStatusKind::Disconnected, Some(error))
            }
            None => (
                EnvironmentStatusKind::Unknown,
                Some(format!("unknown environment id `{environment_id}`")),
            ),
        };
        Ok(Some(EnvironmentStatusResponse { status, error }.into()))
    }
}
