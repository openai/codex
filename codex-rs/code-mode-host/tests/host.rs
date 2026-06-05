use std::process::Stdio;
use std::time::Duration;

use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::wire::ClientMessage;
use codex_code_mode_protocol::wire::HostMessage;
use codex_code_mode_protocol::wire::HostRequest;
use codex_code_mode_protocol::wire::HostResponse;
use codex_code_mode_protocol::wire::read_frame;
use codex_code_mode_protocol::wire::write_frame;
use pretty_assertions::assert_eq;
use tokio::process::Command;

#[tokio::test]
async fn serves_code_mode_sessions_over_stdio() {
    let mut child = Command::new(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")
            .expect("resolve codex-code-mode-host binary"),
    )
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .kill_on_drop(true)
    .spawn()
    .expect("spawn codex-code-mode-host");
    let mut stdin = child.stdin.take().expect("host stdin");
    let mut stdout = child.stdout.take().expect("host stdout");

    write_frame(
        &mut stdin,
        &ClientMessage::Request {
            id: 1,
            request: HostRequest::CreateSession,
        },
    )
    .await
    .expect("create session request");
    let session_id = match read_frame(&mut stdout).await.expect("create response") {
        Some(HostMessage::Response {
            id: 1,
            response: Ok(HostResponse::SessionCreated { session_id }),
        }) => session_id,
        message => panic!("unexpected create-session response: {message:?}"),
    };

    write_frame(
        &mut stdin,
        &ClientMessage::Request {
            id: 2,
            request: HostRequest::Execute {
                session_id,
                request: ExecuteRequest {
                    tool_call_id: "call-1".to_string(),
                    enabled_tools: Vec::new(),
                    source: "text('hello')".to_string(),
                    yield_time_ms: None,
                    max_output_tokens: None,
                },
            },
        },
    )
    .await
    .expect("execute request");
    let cell_id = match read_frame(&mut stdout).await.expect("execute response") {
        Some(HostMessage::Response {
            id: 2,
            response: Ok(HostResponse::ExecutionStarted { cell_id }),
        }) => cell_id,
        message => panic!("unexpected execute response: {message:?}"),
    };
    let response = match read_frame(&mut stdout)
        .await
        .expect("initial response frame")
    {
        Some(HostMessage::InitialResponse {
            id: 2,
            response: Ok(response),
        }) => response,
        message => panic!("unexpected initial response: {message:?}"),
    };
    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id,
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "hello".to_string(),
            }],
            error_text: None,
        }
    );

    write_frame(
        &mut stdin,
        &ClientMessage::Request {
            id: 3,
            request: HostRequest::ShutdownSession { session_id },
        },
    )
    .await
    .expect("shutdown request");
    loop {
        match read_frame(&mut stdout).await.expect("shutdown response") {
            Some(HostMessage::CellClosed {
                session_id: closed_session_id,
                ..
            }) if closed_session_id == session_id => {}
            Some(HostMessage::Response {
                id: 3,
                response: Ok(HostResponse::SessionShutdown),
            }) => break,
            message => panic!("unexpected shutdown response: {message:?}"),
        }
    }

    drop(stdin);
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("host exit timeout")
        .expect("wait for host");
    assert!(status.success(), "host exited with {status}");
}
