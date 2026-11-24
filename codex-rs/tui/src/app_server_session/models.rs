//! Fetch picker models without blocking the event loop and keep new-thread defaults in sync.

use super::AppServerSession;
use super::ThreadParamsMode;
use super::model_preset_from_api_model;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::RequestId;
use codex_protocol::openai_models::ModelPreset;
use uuid::Uuid;

#[derive(Default)]
pub(super) struct StartupModels {
    pub(super) provider: Option<String>,
    pub(super) models: Vec<ModelPreset>,
}

pub(super) async fn startup_list(
    mode: ThreadParamsMode,
    handle: AppServerRequestHandle,
    request_id: RequestId,
    server_version: Option<&str>,
) -> Result<StartupModels, TypedRequestError> {
    if mode == ThreadParamsMode::Cloud {
        // Cloud supplies the model on activation. Discover choices on picker demand.
        return Ok(StartupModels::default());
    }
    // Bind the list to the ID from this bootstrap, even if the default changes mid-request.
    if let Some(provider) = startup_catalog_provider(&handle, server_version).await {
        match list(
            mode,
            handle.clone(),
            request_id.clone(),
            Some(provider.clone()),
        )
        .await
        {
            Ok(models) => {
                return Ok(StartupModels {
                    provider: Some(provider),
                    models,
                });
            }
            Err(TypedRequestError::Server { source, .. }) if source.code == -32602 => {}
            Err(err) => return Err(err),
        }
    }
    let models = list(mode, handle, request_id, /*model_provider*/ None).await?;
    Ok(StartupModels {
        provider: None,
        models,
    })
}

async fn startup_catalog_provider(
    handle: &AppServerRequestHandle,
    server_version: Option<&str>,
) -> Option<String> {
    // Older remote servers (including local daemons) silently ignore modelProvider.
    // Even 0.168 supports modelProvider/list without supporting the selector.
    let version = semver::Version::parse(server_version?).ok()?;
    if version < semver::Version::new(0, 169, 0) {
        return None;
    }
    let page = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 2),
        crate::app::provider::rpc::list_providers(handle),
    )
    .await
    .ok()?
    .ok()?;
    Some(page.default_provider_id)
}

impl AppServerSession {
    pub(crate) fn set_available_models(&mut self, models: Vec<ModelPreset>) {
        if let Some(default) = models
            .iter()
            .find(|model| model.is_default)
            .or(models.first())
        {
            self.default_model = Some(default.model.clone());
        }
        self.available_models = models;
    }

    pub(crate) fn fetch_models(
        &self,
        request_id: Uuid,
        provider: Option<&str>,
        app_event_tx: AppEventSender,
    ) {
        let models = list(
            self.thread_params_mode,
            self.request_handle(),
            RequestId::String(format!("model-list-{request_id}")),
            provider
                .filter(|_| self.thread_params_mode != ThreadParamsMode::Cloud)
                .map(str::to_owned),
        );
        tokio::spawn(async move {
            let result = models.await.map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::ModelsLoaded { request_id, result });
        });
    }
}

async fn list(
    mode: ThreadParamsMode,
    request_handle: AppServerRequestHandle,
    request_id: RequestId,
    model_provider: Option<String>,
) -> Result<Vec<ModelPreset>, TypedRequestError> {
    let response: ModelListResponse = match request_handle
        .request_typed(ClientRequest::ModelList {
            request_id,
            params: ModelListParams {
                cursor: None,
                limit: None,
                include_hidden: Some(true),
                model_provider,
            },
        })
        .await
    {
        Ok(response) => response,
        // Older Cloud servers do not expose model discovery. Preserve their startup behavior.
        Err(TypedRequestError::Server { source, .. })
            if mode == ThreadParamsMode::Cloud && source.code == -32601 =>
        {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    Ok(response
        .data
        .into_iter()
        .map(model_preset_from_api_model)
        .collect())
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;
