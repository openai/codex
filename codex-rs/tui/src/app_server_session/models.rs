//! Fetch picker models without blocking the event loop and keep new-thread defaults in sync.

use super::AppServerSession;
use super::model_preset_from_api_model;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::RequestId;
use codex_protocol::openai_models::ModelPreset;
use uuid::Uuid;

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

    pub(crate) fn fetch_models(&self, request_id: Uuid, app_event_tx: AppEventSender) {
        let request_handle = self.request_handle();
        tokio::spawn(async move {
            let result = request_handle
                .request_typed::<ModelListResponse>(ClientRequest::ModelList {
                    request_id: RequestId::String(format!("model-list-{request_id}")),
                    params: ModelListParams {
                        cursor: None,
                        limit: None,
                        include_hidden: Some(true),
                        model_provider: None,
                    },
                })
                .await
                .map(|response| {
                    response
                        .data
                        .into_iter()
                        .map(model_preset_from_api_model)
                        .collect()
                })
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::ModelsLoaded { request_id, result });
        });
    }
}
