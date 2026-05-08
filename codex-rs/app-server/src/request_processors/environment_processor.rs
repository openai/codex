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
        let response = codex_core_api::environment_add(&self.environment_manager, params)
            .map_err(|err| invalid_request(err.to_string()))?;
        Ok(Some(response.into()))
    }
}
