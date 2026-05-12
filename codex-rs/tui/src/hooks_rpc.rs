use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::HookMetadata;
use codex_app_server_protocol::HookTrustStatus;
use codex_app_server_protocol::HooksListEntry;
use codex_app_server_protocol::HooksListParams;
use codex_app_server_protocol::HooksListResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::RequestId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HookTrustUpdate {
    pub(crate) key: String,
    pub(crate) current_hash: String,
}

pub(crate) async fn fetch_hooks_list(
    request_handle: AppServerRequestHandle,
    cwd: PathBuf,
) -> Result<HooksListResponse> {
    let request_id = RequestId::String(format!("hooks-list-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::HooksList {
            request_id,
            params: HooksListParams { cwds: vec![cwd] },
        })
        .await
        .wrap_err("hooks/list failed in TUI")
}

pub(crate) fn hooks_list_entry_for_cwd(response: HooksListResponse, cwd: &Path) -> HooksListEntry {
    response
        .data
        .into_iter()
        .find(|entry| entry.cwd.as_path() == cwd)
        .unwrap_or_else(|| HooksListEntry {
            cwd: cwd.to_path_buf(),
            hooks: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        })
}

pub(crate) fn hook_needs_review(hook: &HookMetadata) -> bool {
    matches!(
        hook.trust_status,
        HookTrustStatus::Untrusted | HookTrustStatus::Modified
    )
}

pub(crate) async fn write_hook_trusts(
    request_handle: AppServerRequestHandle,
    trust_updates: Vec<HookTrustUpdate>,
) -> Result<ConfigWriteResponse> {
    let request_id = RequestId::String(format!("hooks-config-write-{}", Uuid::new_v4()));
    let value = serde_json::Value::Object(
        trust_updates
            .into_iter()
            .map(|update| {
                (
                    update.key,
                    serde_json::json!({
                        "trusted_hash": update.current_hash,
                    }),
                )
            })
            .collect(),
    );
    request_handle
        .request_typed(ClientRequest::ConfigBatchWrite {
            request_id,
            params: ConfigBatchWriteParams {
                edits: vec![codex_app_server_protocol::ConfigEdit {
                    key_path: "hooks.state".to_string(),
                    value,
                    merge_strategy: MergeStrategy::Upsert,
                }],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            },
        })
        .await
        .wrap_err("config/batchWrite failed while updating hook trust in TUI")
}

pub(crate) async fn write_hook_trust(
    request_handle: AppServerRequestHandle,
    key: String,
    current_hash: String,
) -> Result<ConfigWriteResponse> {
    write_hook_trusts(request_handle, vec![HookTrustUpdate { key, current_hash }]).await
}
