//! Stage bounded TUI diagnostics on the app-server host for a single feedback upload.
//! Log consent gates all staging; cleanup runs after staging or upload failures too.

use crate::app_server_session::fs::AppServerFileSystem;
use codex_app_server_client::AppServerPath;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::FeedbackUploadParams;
use codex_app_server_protocol::FeedbackUploadResponse;
use codex_app_server_protocol::RequestId;
use codex_feedback::CodexFeedback;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use color_eyre::eyre::eyre;
use std::path::PathBuf;
use uuid::Uuid;

// Base64 adds only a third to this size, keeping fs/writeFile below the 16 MiB frame limit.
const MAX_CLIENT_LOG_SIZE: usize = 1024 * 1024;

pub(super) async fn fetch_feedback_upload(
    request_handle: AppServerRequestHandle,
    codex_home: Option<AppServerPath>,
    mut params: FeedbackUploadParams,
    feedback: CodexFeedback,
) -> Result<FeedbackUploadResponse> {
    let fs = AppServerFileSystem { request_handle };
    let staging_dir = codex_home.filter(|_| params.include_logs).map(|home| {
        home.join("tmp")
            .join(format!("tui-feedback-{}", Uuid::new_v4()))
    });
    if params.include_logs {
        let staged = async {
            let directory = staging_dir.as_ref().ok_or_else(|| {
                eyre!("App server did not report $CODEX_HOME; cannot stage feedback logs")
            })?;
            // These RPCs use the app-server's local filesystem, where feedback/upload
            // reads extraLogFiles, independently of the thread's executor environment.
            fs.fs_create_directory_all_path(directory).await?;
            let path = directory.join("client-logs.txt");
            let mut logs = feedback
                .snapshot(/*session_id*/ None)
                .log_attachment(/*logs_override*/ None)
                .buffer;
            logs.drain(..logs.len().saturating_sub(MAX_CLIENT_LOG_SIZE));
            fs.fs_write_file_path(&path, logs).await?;
            Ok::<_, color_eyre::Report>(path)
        }
        .await;
        match staged {
            Ok(path) => params
                .extra_log_files
                .get_or_insert_with(Vec::new)
                .push(PathBuf::from(path.as_str())),
            Err(err) => {
                tracing::warn!("failed to stage TUI feedback logs: {err}");
                params.reason.get_or_insert_with(String::new).push_str(
                    "\n\nTUI client logs were omitted because they could not be staged on the app-server.",
                );
            }
        }
    }
    let request_id = RequestId::String(format!("feedback-upload-{}", Uuid::new_v4()));
    let result = fs
        .request_handle
        .request_typed(ClientRequest::FeedbackUpload { request_id, params })
        .await
        .wrap_err("feedback/upload failed in TUI");

    if let Some(directory) = staging_dir
        && let Err(err) = fs.fs_remove_path(&directory).await
    {
        tracing::warn!("failed to remove staged feedback logs: {err}");
    }
    result
}
