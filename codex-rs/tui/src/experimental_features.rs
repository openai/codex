//! Bounded experimental-feature discovery and persistence through existing RPCs.
//! Dropping a popup stops discovery, but submitted writes finish independently.
//! Readback describes configured enablement; reserved IDs bound unanswered requests.

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::ExperimentalFeature;
use codex_app_server_protocol::ExperimentalFeatureListParams;
use codex_app_server_protocol::ExperimentalFeatureListResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WriteStatus;
use codex_protocol::ThreadId;
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::oneshot;

pub(crate) fn fetch(
    request_handle: AppServerRequestHandle,
    thread_id: Option<ThreadId>,
    request_id: &'static str,
    mut response_tx: oneshot::Sender<Result<Vec<ExperimentalFeature>, String>>,
) {
    tokio::spawn(async move {
        let discovery = async {
            let mut features = Vec::new();
            let mut cursor = None;
            let mut cursors = HashSet::new();
            let mut names = HashSet::new();
            for _ in 0..10 {
                let response = request_handle
                    .request_typed::<ExperimentalFeatureListResponse>(
                        ClientRequest::ExperimentalFeatureList {
                            // The client rejects duplicate pending IDs, bounding unanswered
                            // requests across popup cancellation and timeout/retry cycles.
                            request_id: RequestId::String(request_id.to_string()),
                            params: ExperimentalFeatureListParams {
                                cursor,
                                limit: Some(100),
                                thread_id: thread_id.map(|id| id.to_string()),
                            },
                        },
                    )
                    .await
                    .map_err(|_| "Experimental feature request failed".to_string())?;
                if response.data.len() > 100 {
                    return Err("Experimental feature page exceeds requested limit".to_string());
                }
                features.extend(
                    response
                        .data
                        .into_iter()
                        .filter(|feature| names.insert(feature.name.clone())),
                );
                cursor = response.next_cursor;
                let Some(next) = cursor.as_ref() else {
                    return Ok(features);
                };
                if !cursors.insert(next.clone()) {
                    return Err("Experimental feature pagination repeated a cursor".to_string());
                }
            }
            Err("Experimental feature discovery exceeded 10 pages".to_string())
        };
        tokio::select! {
            _ = response_tx.closed() => {},
            result = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), discovery) => {
                let result = result.unwrap_or_else(|_| Err("Experimental feature discovery timed out".to_string()));
                let _ = response_tx.send(result);
            }
        }
    });
}

/// Configured readback, deliberately separate from running-task feature state.
#[derive(Debug)]
pub(crate) struct FeatureWriteResult {
    pub features: Vec<ExperimentalFeature>,
    pub warning: Option<String>,
}

pub(crate) async fn write(
    request_handle: AppServerRequestHandle,
    thread_id: ThreadId,
    updates: Vec<(String, bool)>,
) -> Result<FeatureWriteResult, String> {
    let (tx, rx) = oneshot::channel();
    fetch(
        request_handle.clone(),
        Some(thread_id),
        "tui-experimental-save-readback",
        tx,
    );
    let features = rx
        .await
        .map_err(|_| "Feature discovery was interrupted")??;
    let edits = updates
        .iter()
        .map(|(name, enabled)| {
            let feature = features
                .iter()
                .find(|feature| feature.name == *name)
                .ok_or_else(|| {
                    format!("The server did not advertise experimental feature `{name}`")
                })?;
            // Quote the server's key as a single TOML path segment.
            let key = format!("features.{}", serde_json::json!(name));
            Ok(crate::config_update::replace_config_value(
                key,
                // Keep the daemon opt-out explicit even before rollout defaults enable it.
                if *enabled || feature.default_enabled || name == "daemon_auto_start" {
                    serde_json::json!(enabled)
                } else {
                    serde_json::Value::Null
                },
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let response = tokio::time::timeout(Duration::from_secs(/*secs*/ 15), request_handle
        .request_typed::<ConfigWriteResponse>(ClientRequest::ConfigBatchWrite {
            // A timed-out write may still finish. Bound unanswered retries too.
            request_id: RequestId::String("tui-experimental-feature-write".to_string()),
            params: ConfigBatchWriteParams {
                edits,
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            },
        })).await
        .map_err(|_| "Saving experimental features timed out; the write may still finish. Reopen /experimental to check.")?
        .map_err(|_| "Failed to save experimental features. Reopen /experimental to check configured values before retrying.")?;
    let (tx, rx) = oneshot::channel();
    fetch(
        request_handle,
        Some(thread_id),
        "tui-experimental-save-readback",
        tx,
    );
    let features = rx
        .await
        .map_err(|_| "Features were saved, but readback was interrupted")?
        .map_err(|error| {
            format!("Features were saved, but configured values could not be refreshed: {error}")
        })?;
    let overridden = response.status == WriteStatus::OkOverridden
        || updates.iter().any(|(name, enabled)| {
            !features
                .iter()
                .any(|feature| feature.name == *name && feature.enabled == *enabled)
        });
    Ok(FeatureWriteResult {
        features,
        warning: overridden.then(|| "Changes were saved, but the configured values differ from your selections. A higher-priority setting may override them.".to_string()),
    })
}

#[cfg(test)]
#[path = "experimental_features_tests.rs"]
mod tests;
