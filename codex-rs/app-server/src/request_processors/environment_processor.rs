use super::*;
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
        let result = match params.connect_timeout_ms {
            Some(connect_timeout_ms) => self
                .environment_manager
                .upsert_environment_with_connect_timeout(
                    params.environment_id,
                    params.exec_server_url,
                    Duration::from_millis(connect_timeout_ms),
                ),
            None => self
                .environment_manager
                .upsert_environment(params.environment_id, params.exec_server_url),
        };
        result.map_err(|err| invalid_request(err.to_string()))?;
        Ok(Some(EnvironmentAddResponse {}.into()))
    }
}
