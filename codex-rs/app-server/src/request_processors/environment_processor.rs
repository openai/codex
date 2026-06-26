use super::*;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct EnvironmentRequestProcessor {
    environment_manager: Arc<EnvironmentManager>,
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
}

impl EnvironmentRequestProcessor {
    pub(crate) fn new(
        environment_manager: Arc<EnvironmentManager>,
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
    ) -> Self {
        Self {
            environment_manager,
            thread_manager,
            outgoing,
        }
    }

    pub(crate) async fn environment_add(
        &self,
        params: EnvironmentAddParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let environment_id = params.environment_id;
        self.environment_manager
            .upsert_environment(
                environment_id.clone(),
                params.exec_server_url,
                params.connect_timeout_ms.map(Duration::from_millis),
            )
            .map_err(|err| invalid_request(err.to_string()))?;
        let environment = self
            .environment_manager
            .get_environment(&environment_id)
            .expect("upserted environment should be available");
        let thread_manager = Arc::clone(&self.thread_manager);
        let outgoing = Arc::clone(&self.outgoing);
        tokio::spawn(async move {
            if environment.wait_until_ready().await.is_err() {
                return;
            }
            for thread_id in thread_manager.list_thread_ids().await {
                let Ok(thread) = thread_manager.get_thread(thread_id).await else {
                    continue;
                };
                let selected_environment = thread.selected_capability_roots().iter().any(|root| {
                    matches!(
                        &root.location,
                        codex_protocol::capabilities::CapabilityRootLocation::Environment {
                            environment_id: selected_environment_id,
                            ..
                        } if selected_environment_id == &environment_id
                    )
                });
                if selected_environment {
                    crate::extensions::send_thread_skills_changed(&outgoing, thread_id).await;
                }
            }
        });
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
        let info = environment.info().await.map_err(|err| {
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
}
