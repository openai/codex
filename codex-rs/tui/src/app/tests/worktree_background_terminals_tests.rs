use super::*;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ThreadBackgroundTerminalsListResponse;
use pretty_assertions::assert_eq;

#[test]
fn old_local_daemon_worktree_error_suggests_update() -> Result<()> {
    let target = AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        },
    };
    let missing_method = |code, message: &str| {
        Err(TypedRequestError::Server {
            method: "thread/backgroundTerminals/list".to_string(),
            source: JSONRPCErrorError {
                code,
                message: message.to_string(),
                data: None,
            },
        })
    };
    let message = managed_worktree_creation::background_terminals_blocker(
        missing_method(-32601, "method not found"),
        &target,
    )
    .expect("unsupported daemon must block worktree creation");
    let cell = crate::history_cell::new_error_event(message.to_string());
    insta::assert_snapshot!(
        lines_to_single_string(&cell.display_lines(/*width*/ 120)),
        @"■ The local Codex service cannot check background terminals. Run `codex app-server daemon update`, then restart Codex."
    );
    assert_eq!(
        managed_worktree_creation::background_terminals_blocker(
            missing_method(
                -32600,
                "Invalid request: unknown variant `thread/backgroundTerminals/list`"
            ),
            &target,
        ),
        Some(message),
    );
    assert_eq!(
        managed_worktree_creation::background_terminals_blocker(
            missing_method(-32600, "Invalid request: unknown variant `another/method`"),
            &target,
        ),
        Some("Active background terminals block /cd."),
    );
    assert_eq!(
        managed_worktree_creation::background_terminals_blocker(
            missing_method(-32601, "method not found"),
            &AppServerTarget::Embedded,
        ),
        Some("Active background terminals block /cd."),
    );
    assert_eq!(
        managed_worktree_creation::background_terminals_blocker(
            Ok(ThreadBackgroundTerminalsListResponse {
                data: Vec::new(),
                next_cursor: None,
            }),
            &target,
        ),
        None,
    );
    Ok(())
}
