use super::*;
use crate::app_event::SideThreadPrepareError;
use crate::app_server_session::ThreadParamsMode;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadInjectItemsParams;
use codex_app_server_protocol::ThreadInjectItemsResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;

pub(super) async fn prepare_side_thread(
    request_handle: AppServerRequestHandle,
    config: Config,
    parent_thread_id: ThreadId,
    thread_params_mode: ThreadParamsMode,
    remote_cwd_override: Option<PathBuf>,
) -> std::result::Result<AppServerStartedThread, SideThreadPrepareError> {
    let boundary_item = serde_json::to_value(App::side_boundary_prompt_item()).map_err(|err| {
        SideThreadPrepareError {
            thread_id: None,
            message: format!("failed to encode thread/inject_items payload: {err}"),
        }
    })?;
    let started = crate::app_server_session::fork_thread_with_request_handle(
        request_handle.clone(),
        config,
        parent_thread_id,
        thread_params_mode,
        remote_cwd_override,
    )
    .await
    .map_err(|err| SideThreadPrepareError {
        thread_id: None,
        message: format!("{err:#}"),
    })?;
    let child_thread_id = started.session.thread_id;

    // Keep fork and boundary injection in one background operation so the App never observes a
    // side thread that can run before its inherited history is marked reference-only.
    let inject_result = request_handle
        .request_typed::<ThreadInjectItemsResponse>(ClientRequest::ThreadInjectItems {
            request_id: RequestId::String(format!("side-thread-inject-items-{}", Uuid::new_v4())),
            params: ThreadInjectItemsParams {
                thread_id: child_thread_id.to_string(),
                items: vec![boundary_item],
            },
        })
        .await;
    if let Err(err) = inject_result {
        return Err(SideThreadPrepareError {
            thread_id: Some(child_thread_id),
            message: format!(
                "thread/inject_items failed during TUI side conversation setup: {err}"
            ),
        });
    }
    Ok(started)
}

pub(super) async fn unsubscribe_side_thread(
    request_handle: AppServerRequestHandle,
    thread_id: ThreadId,
) -> std::result::Result<(), String> {
    request_handle
        .request_typed::<ThreadUnsubscribeResponse>(ClientRequest::ThreadUnsubscribe {
            request_id: RequestId::String(format!("side-thread-unsubscribe-{}", Uuid::new_v4())),
            params: ThreadUnsubscribeParams {
                thread_id: thread_id.to_string(),
            },
        })
        .await
        .map(|_| ())
        .map_err(|err| format!("thread/unsubscribe failed in TUI: {err}"))
}
