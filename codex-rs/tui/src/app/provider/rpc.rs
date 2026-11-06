//! List providers and select a default from either settings or sign-in.

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::ModelProviderListParams;
use codex_app_server_protocol::ModelProviderListResponse;
use codex_app_server_protocol::RequestId;
use uuid::Uuid;

pub(crate) async fn list_providers(
    handle: &AppServerRequestHandle,
) -> Result<ModelProviderListResponse, String> {
    handle
        .request_typed(ClientRequest::ModelProviderList {
            request_id: RequestId::String(Uuid::new_v4().to_string()),
            params: ModelProviderListParams {},
        })
        .await
        .map_err(request_error)
}

pub(crate) async fn set_default_provider(
    handle: &AppServerRequestHandle,
    id: &str,
) -> Result<ConfigWriteResponse, TypedRequestError> {
    handle
        .request_typed(ClientRequest::ConfigValueWrite {
            request_id: RequestId::String(Uuid::new_v4().to_string()),
            params: ConfigValueWriteParams {
                key_path: "model_provider".into(),
                value: id.into(),
                merge_strategy: MergeStrategy::Replace,
                file_path: None,
                expected_version: None,
            },
        })
        .await
}

pub(crate) fn override_reason(response: ConfigWriteResponse) -> String {
    response
        .overridden_metadata
        .map(|metadata| metadata.message)
        .unwrap_or_else(|| "A higher-priority setting overrides this choice.".into())
}

pub(super) fn request_error(err: TypedRequestError) -> String {
    if let TypedRequestError::Server { method, source } = &err
        && (source.code == -32601
            || (source.code == -32600
                && source
                    .message
                    .starts_with(&format!("Invalid request: unknown variant `{method}`"))))
    {
        return "Update the connected app server to browse model providers.".into();
    }
    format!("Could not load model providers: {err}")
}
