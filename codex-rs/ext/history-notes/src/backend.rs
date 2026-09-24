use std::time::Duration;

use codex_client::HttpTransport;
use codex_client::RequestBody;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_login::default_client::create_transport_for_routes_async;
use codex_model_provider::SharedModelProvider;
use codex_utils_output_truncation::TruncationPolicy;
use http::HeaderValue;
use http::Method;
use serde_json::Value;
use serde_json::json;

const HISTORY_NOTES_BACKEND_TIMEOUT: Duration = Duration::from_secs(35);
const ENCRYPTED_TOOL_ARGUMENTS_HEADER: &str = "x-openai-encrypted-tool-arguments";
const TOOL_OUTPUT_TRUNCATION_POLICY_HEADER: &str = "x-openai-tool-output-truncation-policy";
const OPERATION_ERROR_PREFIX: &str = "Unable to perform operation:";

#[derive(Clone)]
pub(crate) struct HistoryNotesBackend {
    provider: SharedModelProvider,
    http_client_factory: HttpClientFactory,
}

impl HistoryNotesBackend {
    pub(crate) fn new(
        provider: SharedModelProvider,
        http_client_factory: HttpClientFactory,
    ) -> Self {
        Self {
            provider,
            http_client_factory,
        }
    }

    pub(crate) async fn call(
        &self,
        path: &str,
        session_id: &str,
        current_agent_name: &str,
        mut arguments: Value,
        truncation_policy: TruncationPolicy,
    ) -> Result<Value, String> {
        let Some(arguments_object) = arguments.as_object_mut() else {
            return Err("History tool arguments must be a JSON object".to_string());
        };
        arguments_object.insert(
            "context".to_string(),
            json!({
                "session_id": session_id,
                "current_agent_name": current_agent_name,
            }),
        );

        let provider = self.provider.api_provider().await.map_err(|_| {
            format!("{OPERATION_ERROR_PREFIX} Could not resolve the backend provider.")
        })?;
        let auth = self.provider.api_auth().await.map_err(|_| {
            format!("{OPERATION_ERROR_PREFIX} Could not resolve backend authentication.")
        })?;

        let mut request = provider.build_request(Method::POST, path);
        let encoded_truncation_policy =
            serde_json::to_string(&truncation_policy).map_err(|_| {
                format!("{OPERATION_ERROR_PREFIX} Could not encode the output truncation policy.")
            })?;
        request.headers.insert(
            TOOL_OUTPUT_TRUNCATION_POLICY_HEADER,
            HeaderValue::from_str(&encoded_truncation_policy).map_err(|_| {
                format!(
                    "{OPERATION_ERROR_PREFIX} Could not construct the output truncation policy header."
                )
            })?,
        );
        if matches!(
            path,
            "alpha/history/v2/search_contents"
                | "alpha/notes/v2/search_contents"
                | "alpha/notes/v2/append_to_file"
                | "alpha/notes/v2/write_file"
        ) {
            request.headers.insert(
                ENCRYPTED_TOOL_ARGUMENTS_HEADER,
                HeaderValue::from_static("true"),
            );
        }
        request.body = Some(RequestBody::Json(arguments));
        request.timeout = Some(HISTORY_NOTES_BACKEND_TIMEOUT);
        let request = auth.apply_auth(request).await.map_err(|_| {
            format!("{OPERATION_ERROR_PREFIX} Could not apply backend authentication.")
        })?;
        let transport = create_transport_for_routes_async(
            self.http_client_factory.clone(),
            ClientRouteClass::Api,
        )
        .await
        .map_err(|_| format!("{OPERATION_ERROR_PREFIX} The backend request failed."))?;
        let response = transport
            .execute(request)
            .await
            .map_err(|_| format!("{OPERATION_ERROR_PREFIX} The backend request failed."))?;

        serde_json::from_slice(&response.body)
            .map_err(|_| format!("{OPERATION_ERROR_PREFIX} The backend returned invalid JSON."))
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
