//! Execution boundary for history and notes. Each backend selects the wire representation
//! it accepts for a tool. The HTTP proxy preserves catalog-defined arguments verbatim.

use crate::requests::Namespace;
use crate::tools::ToolResponse;
use codex_tools::ToolExposure;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::future::Future;
use std::sync::Arc;
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

#[derive(Serialize)]
pub(crate) struct HttpContext {
    pub(crate) session_id: String,
    pub(crate) current_agent_name: String,
}

/// Metadata shared by schema registration and the HTTP proxy. The catalog may override
/// model-facing descriptions and parameters, while the endpoint stays harness-owned.
pub(crate) trait Request: DeserializeOwned + JsonSchema + Send + Sync + 'static {
    const NAMESPACE: Namespace;
    const NAME: &'static str;
    const PARALLEL: bool = true;
}

/// Executes each tool using the wire representation its backend can read. The HTTP
/// proxy forwards opaque JSON. A typed backend uses the request and its declared response.
pub(crate) trait Backend: Clone + Send + Sync + 'static {
    const EXPOSURE: ToolExposure;
    type Input<R: Request>: DeserializeOwned + Send;
}

/// Implements one operation. Each backend chooses its result type for this request.
pub(crate) trait Handle<R: Request>: Backend {
    type Output: ToolResponse;

    fn handle(
        &self,
        budget: TruncationPolicy,
        input: Self::Input<R>,
    ) -> impl Future<Output = Result<Self::Output, String>> + Send;
}

#[derive(Clone)]
pub(crate) struct HttpBackend {
    provider: SharedModelProvider,
    http_client_factory: HttpClientFactory,
    context: Arc<HttpContext>,
}

impl HttpBackend {
    pub(crate) fn new(
        provider: SharedModelProvider,
        http_client_factory: HttpClientFactory,
        context: Arc<HttpContext>,
    ) -> Self {
        Self {
            provider,
            http_client_factory,
            context,
        }
    }

    pub(crate) async fn call(
        &self,
        path: &str,
        mut arguments: Value,
        truncation_policy: TruncationPolicy,
    ) -> Result<Value, String> {
        let Some(arguments_object) = arguments.as_object_mut() else {
            return Err("History tool arguments must be a JSON object".to_string());
        };
        arguments_object.insert("context".to_string(), json!(self.context.as_ref()));

        let setup = codex_model_provider::resolve_request(
            self.provider.as_ref(),
            /*scope*/ None,
            /*routing*/ None,
            self.http_client_factory.clone(),
        )
        .await
        .map_err(|_| format!("{OPERATION_ERROR_PREFIX} Could not prepare the backend request."))?;

        let mut request = setup.api_provider.build_request(Method::POST, path);
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
        let request = setup.api_auth.apply_auth(request).await.map_err(|_| {
            format!("{OPERATION_ERROR_PREFIX} Could not apply backend authentication.")
        })?;
        let transport = create_transport_for_routes_async(
            setup.http_client_factory,
            ClientRouteClass::Api,
            setup.redirect_policy,
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

impl Backend for HttpBackend {
    const EXPOSURE: ToolExposure = ToolExposure::DirectModelOnly;

    type Input<R: Request> = Value;
}

impl<R: Request> Handle<R> for HttpBackend {
    type Output = Value;

    async fn handle(&self, budget: TruncationPolicy, input: Value) -> Result<Value, String> {
        let path = format!("alpha/{}/v2/{}", R::NAMESPACE.name, R::NAME);
        self.call(&path, input, budget).await
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
