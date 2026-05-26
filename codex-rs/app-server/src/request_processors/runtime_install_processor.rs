use super::*;
use codex_app_server_protocol::RuntimeInstallCancelResponse;
use codex_app_server_protocol::RuntimeInstallCancelStatus;
use codex_app_server_protocol::RuntimeInstallParams;
use codex_app_server_protocol::RuntimeInstallProgressNotification;
use codex_app_server_protocol::RuntimeInstallProgressPhase;
use std::sync::Mutex as StdMutex;

#[derive(Clone)]
pub(crate) struct RuntimeInstallRequestProcessor {
    environment_manager: Arc<EnvironmentManager>,
    outgoing: Arc<OutgoingMessageSender>,
    thread_manager: Arc<ThreadManager>,
    active_install: Arc<StdMutex<Option<CancellationToken>>>,
}

impl RuntimeInstallRequestProcessor {
    pub(crate) fn new(
        environment_manager: Arc<EnvironmentManager>,
        outgoing: Arc<OutgoingMessageSender>,
        thread_manager: Arc<ThreadManager>,
    ) -> Self {
        Self {
            environment_manager,
            outgoing,
            thread_manager,
            active_install: Arc::new(StdMutex::new(None)),
        }
    }

    pub(crate) async fn install_runtime(
        &self,
        connection_id: ConnectionId,
        mut params: RuntimeInstallParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let (cancellation, _active_install) = self.begin_install()?;
        let environment = if let Some(environment_id) = params.environment_id.take() {
            self.environment_manager
                .get_environment(&environment_id)
                .ok_or_else(|| {
                    invalid_request(format!(
                        "unknown runtime install environment id `{environment_id}`"
                    ))
                })?
        } else {
            self.environment_manager
                .default_or_local_environment()
                .ok_or_else(|| internal_error("runtime install environment is not configured"))?
        };

        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
        let outgoing = Arc::clone(&self.outgoing);
        let progress_forwarder = tokio::spawn(async move {
            while let Some(progress) = progress_rx.recv().await {
                outgoing
                    .send_server_notification_to_connections(
                        &[connection_id],
                        ServerNotification::RuntimeInstallProgress(progress),
                    )
                    .await;
            }
        });

        let install_result = codex_runtime_install::install_runtime_with_progress(
            &environment,
            params,
            progress_tx,
            cancellation,
        )
        .await;
        if let Err(error) = progress_forwarder.await {
            warn!("runtime install progress forwarder failed: {error}");
        }

        let response = install_result?;
        self.send_progress(
            connection_id,
            RuntimeInstallProgressNotification {
                bundle_version: response.bundle_version.clone(),
                downloaded_bytes: None,
                phase: RuntimeInstallProgressPhase::Configuring,
                total_bytes: None,
            },
        )
        .await;
        let response =
            crate::runtime_install::finalize_runtime_install(&environment, response).await?;
        self.thread_manager.plugins_manager().clear_cache();
        self.thread_manager.skills_manager().clear_cache();
        Ok(Some(response.into()))
    }

    pub(crate) async fn cancel_runtime_install(
        &self,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let status = {
            let active_install = self.active_install();
            match active_install.as_ref() {
                Some(cancellation) => {
                    cancellation.cancel();
                    RuntimeInstallCancelStatus::Canceled
                }
                None => RuntimeInstallCancelStatus::NotFound,
            }
        };
        Ok(Some(RuntimeInstallCancelResponse { status }.into()))
    }

    fn begin_install(&self) -> Result<(CancellationToken, ActiveInstallGuard), JSONRPCErrorError> {
        let cancellation = CancellationToken::new();
        let mut active_install = self.active_install();
        if active_install.is_some() {
            return Err(invalid_request("runtime install is already in progress"));
        }
        *active_install = Some(cancellation.clone());
        drop(active_install);
        Ok((
            cancellation,
            ActiveInstallGuard {
                active_install: Arc::clone(&self.active_install),
            },
        ))
    }

    fn active_install(&self) -> std::sync::MutexGuard<'_, Option<CancellationToken>> {
        self.active_install
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn send_progress(
        &self,
        connection_id: ConnectionId,
        progress: RuntimeInstallProgressNotification,
    ) {
        self.outgoing
            .send_server_notification_to_connections(
                &[connection_id],
                ServerNotification::RuntimeInstallProgress(progress),
            )
            .await;
    }
}

struct ActiveInstallGuard {
    active_install: Arc<StdMutex<Option<CancellationToken>>>,
}

impl Drop for ActiveInstallGuard {
    fn drop(&mut self) {
        self.active_install
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}
