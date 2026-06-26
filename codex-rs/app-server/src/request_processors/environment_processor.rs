use super::*;
use codex_exec_server::Environment;
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
        let processor = Self {
            environment_manager,
            thread_manager,
            outgoing,
        };
        for (environment_id, environment) in processor.environment_manager.registered_environments()
        {
            processor.notify_selected_threads_when_ready(environment_id, environment);
        }
        processor
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
            .ok_or_else(|| internal_error("upserted environment is unavailable"))?;
        self.notify_selected_threads_when_ready(environment_id, environment);
        Ok(Some(EnvironmentAddResponse {}.into()))
    }

    fn notify_selected_threads_when_ready(
        &self,
        environment_id: String,
        environment: Arc<Environment>,
    ) {
        let startup = environment.observe_startup();
        let thread_manager = Arc::clone(&self.thread_manager);
        let outgoing = Arc::clone(&self.outgoing);
        tokio::spawn(async move {
            if startup.await.is_err() {
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
