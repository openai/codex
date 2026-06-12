use super::*;

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
        match params.exec_server_url {
            Some(exec_server_url) => self
                .environment_manager
                .upsert_environment(params.environment_id, exec_server_url),
            None => self
                .environment_manager
                .register_pending_environment(params.environment_id)
                .map(drop),
        }
        .map_err(|err| invalid_request(err.to_string()))?;
        Ok(Some(EnvironmentAddResponse {}.into()))
    }
}
