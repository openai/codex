use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::experimental_required_message;
use color_eyre::Report;
use pretty_assertions::assert_eq;

use super::JSONRPC_INVALID_REQUEST;
use super::JSONRPC_METHOD_NOT_FOUND;
use super::THREAD_QUEUE_ADD_METHOD;
use super::is_unsupported_queue_error;

#[test]
fn recognizes_only_definitively_unsupported_queue_errors() {
    for (code, message, unsupported) in [
        (
            JSONRPC_METHOD_NOT_FOUND,
            "Method not found".to_string(),
            true,
        ),
        (
            JSONRPC_INVALID_REQUEST,
            experimental_required_message(THREAD_QUEUE_ADD_METHOD),
            true,
        ),
        (
            JSONRPC_INVALID_REQUEST,
            "Invalid request: unknown variant `thread/queue/add`, expected `thread/list`"
                .to_string(),
            true,
        ),
        (
            JSONRPC_INVALID_REQUEST,
            "queue cannot contain more than 100 submissions".to_string(),
            false,
        ),
    ] {
        let error = Report::new(TypedRequestError::Server {
            method: THREAD_QUEUE_ADD_METHOD.to_string(),
            source: JSONRPCErrorError {
                code,
                message,
                data: None,
            },
        });

        assert_eq!(is_unsupported_queue_error(&error), unsupported);
    }
    let transport_error = Report::new(TypedRequestError::Transport {
        method: THREAD_QUEUE_ADD_METHOD.to_string(),
        source: std::io::Error::new(std::io::ErrorKind::TimedOut, "request timed out"),
    });
    assert!(!is_unsupported_queue_error(&transport_error));
}
