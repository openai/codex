#![cfg(unix)]

mod common;

#[cfg(target_os = "linux")]
use anyhow::Context;
#[cfg(target_os = "linux")]
use codex_exec_server::Environment;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server_protocol::JSONRPCError;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
#[cfg(target_os = "linux")]
use codex_utils_absolute_path::AbsolutePathBuf;
#[cfg(target_os = "linux")]
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;
#[cfg(target_os = "linux")]
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WebSocketError;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::http::header::ORIGIN;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_reports_malformed_websocket_json_and_keeps_running() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    server.send_raw_text("not-json").await?;

    let response = server
        .wait_for_event(|event| matches!(event, JSONRPCMessage::Error(_)))
        .await?;
    let JSONRPCMessage::Error(JSONRPCError { id, error }) = response else {
        panic!("expected malformed-message error response");
    };
    assert_eq!(id, codex_exec_server_protocol::RequestId::Integer(-1));
    assert_eq!(error.code, -32600);
    assert!(
        error
            .message
            .starts_with("failed to parse websocket JSON-RPC message from exec-server websocket"),
        "unexpected malformed-message error: {}",
        error.message
    );

    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;

    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &initialize_id
            )
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected initialize response after malformed input");
    };
    assert_eq!(id, initialize_id);
    let initialize_response: InitializeResponse = serde_json::from_value(result)?;
    Uuid::parse_str(&initialize_response.session_id)?;

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_accepts_binary_websocket_json() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let initialize_id = codex_exec_server_protocol::RequestId::Integer(1);
    let initialize = JSONRPCMessage::Request(codex_exec_server_protocol::JSONRPCRequest {
        id: initialize_id.clone(),
        method: "initialize".to_string(),
        params: Some(serde_json::to_value(InitializeParams {
            client_name: "exec-server-binary-test".to_string(),
            resume_session_id: None,
        })?),
        trace: None,
    });
    server
        .send_raw_binary(serde_json::to_vec(&initialize)?)
        .await?;

    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &initialize_id
            )
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected initialize response for binary input");
    };
    assert_eq!(id, initialize_id);
    let initialize_response: InitializeResponse = serde_json::from_value(result)?;
    Uuid::parse_str(&initialize_response.session_id)?;

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_echoes_request_id_in_websocket_handshake() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    for request_id in [Some(HeaderValue::from_static("opaque-request-id")), None] {
        let mut request = server.websocket_url().into_client_request()?;
        if let Some(request_id) = &request_id {
            request
                .headers_mut()
                .insert("x-request-id", request_id.clone());
        }

        let (stream, response) = connect_async(request).await?;
        assert_eq!(response.headers().get("x-request-id"), request_id.as_ref());
        drop(stream);
    }

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_rejects_browser_origin_websocket_handshake() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let mut request = server.websocket_url().into_client_request()?;
    request
        .headers_mut()
        .insert(ORIGIN, HeaderValue::from_static("https://evil.example"));

    let error = match connect_async(request).await {
        Ok(_) => anyhow::bail!("browser-origin websocket handshake should be rejected"),
        Err(error) => error,
    };
    let WebSocketError::Http(response) = error else {
        anyhow::bail!("browser-origin websocket handshake failed unexpectedly: {error}");
    };
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    server.shutdown().await?;
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_metadata_watch_invalidates_on_idle_websocket_and_recovery() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let mut proxy = server.disconnectable_websocket_proxy().await?;
    let environment = Environment::create_for_tests(Some(proxy.websocket_url().to_owned()))?;
    let fs = environment.get_filesystem();
    let repo = tempfile::tempdir()?;
    let root = AbsolutePathBuf::try_from(repo.path().canonicalize()?)?;
    let skills = root.join(".agents/skills");
    let marker = root.join(".git");
    std::fs::write(&marker, "")?;
    let watch = || {
        fs.watch_missing(
            vec![PathUri::from_abs_path(&skills)],
            Some(PathUri::from_abs_path(&marker)),
        )
    };
    let superseded = watch().await?.context("first watch")?;
    let mut invalidated = watch()
        .await?
        .context("executor accepted empty observation")?;
    assert!(superseded.await.is_err());
    std::fs::write(root.join(".agents-backup"), "")?;
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut invalidated)
            .await
            .is_err()
    );
    std::fs::create_dir_all(&skills)?;
    // The executor pushes this across the already open socket, without another RPC.
    tokio::time::timeout(Duration::from_secs(5), invalidated).await??;
    assert!(watch().await?.is_none());

    std::fs::remove_dir(&skills)?;
    let invalidated = watch().await?.context("watch after deletion")?;
    std::fs::remove_file(&marker)?;
    tokio::time::timeout(Duration::from_secs(5), invalidated).await??;
    assert!(watch().await?.is_none());

    std::fs::write(&marker, "")?;
    let old = watch().await?.context("watch before disconnect")?;
    proxy.pause_and_disconnect().await?;
    assert!(
        tokio::time::timeout(Duration::from_secs(5), old)
            .await?
            .is_err()
    );
    std::fs::create_dir(&skills)?;
    proxy.resume()?;
    assert!(watch().await?.is_none());
    std::fs::remove_dir(&skills)?;
    let new = watch()
        .await?
        .context("new watch on recovered connection")?;
    std::fs::create_dir(&skills)?;
    tokio::time::timeout(Duration::from_secs(5), new).await??;

    server.shutdown().await?;
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_metadata_watch_invalidates_on_possible_casefold_aliases() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let environment = Environment::create_for_tests(Some(server.websocket_url().to_owned()))?;
    let fs = environment.get_filesystem();

    // U+017F (long s) folds to ASCII s in a casefolded ext4 directory.
    for (parent, missing, created) in [
        ("", ".agents/skills", ".Agents"),
        (".agents", "skills", "Skills"),
        (".agents", "skills", "\u{017f}kills"),
        (".agents", "\u{017f}kills", "skills"),
    ] {
        let repo = tempfile::tempdir()?;
        let root = AbsolutePathBuf::try_from(repo.path().canonicalize()?)?;
        let parent = root.join(parent);
        std::fs::create_dir_all(&parent)?;
        let missing = PathUri::from_abs_path(&parent.join(missing));
        let invalidated = fs
            .watch_missing(vec![missing], None)
            .await?
            .context("executor accepted empty observation")?;

        std::fs::create_dir(parent.join(created))?;
        tokio::time::timeout(Duration::from_secs(5), invalidated)
            .await
            .with_context(|| format!("watch missed {created}"))??;
    }

    server.shutdown().await?;
    Ok(())
}
