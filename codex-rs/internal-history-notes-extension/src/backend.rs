use std::time::Duration;

use codex_api::ReqwestTransport;
use codex_client::HttpTransport;
use codex_client::RequestBody;
use codex_login::default_client::create_client;
use codex_model_provider::SharedModelProvider;
use http::Method;
use serde_json::Value;

const HISTORY_NOTES_BACKEND_TIMEOUT: Duration = Duration::from_secs(35);

#[derive(Clone)]
pub(crate) struct HistoryNotesBackend {
    provider: SharedModelProvider,
}

impl HistoryNotesBackend {
    pub(crate) fn new(provider: SharedModelProvider) -> Self {
        Self { provider }
    }

    pub(crate) async fn call(
        &self,
        path: &str,
        current_thread_id: &str,
        mut arguments: Value,
    ) -> Result<Value, String> {
        let Some(arguments_object) = arguments.as_object_mut() else {
            return Err("History tool arguments must be a JSON object".to_string());
        };
        arguments_object.insert(
            "current_thread_id".to_string(),
            Value::String(current_thread_id.to_string()),
        );

        let provider =
            self.provider.api_provider().await.map_err(|error| {
                format!("History backend provider could not be resolved: {error}")
            })?;
        let auth = self
            .provider
            .api_auth()
            .await
            .map_err(|error| format!("History backend auth could not be resolved: {error}"))?;

        let mut request = provider.build_request(Method::POST, path);
        request.body = Some(RequestBody::Json(arguments));
        request.timeout = Some(HISTORY_NOTES_BACKEND_TIMEOUT);
        let request = auth
            .apply_auth(request)
            .await
            .map_err(|error| format!("History backend auth failed: {error}"))?;
        let response = ReqwestTransport::from_http_client(create_client())
            .execute(request)
            .await
            .map_err(|error| format!("History backend request failed: {error}"))?;

        serde_json::from_slice(&response.body)
            .map_err(|error| format!("History backend returned invalid JSON: {error}"))
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
