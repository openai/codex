#![cfg(test)]
//! Exercises optional search progress and cancellation over the public JSON-RPC transport.

use super::connection_handling_websocket as ws;
use anyhow::Context;
use anyhow::Result;
use app_test_support::create_fake_rollout_with_source;
use app_test_support::rollout_path;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadSearchPartialNotification;
use codex_app_server_protocol::ThreadSearchResponse;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

struct SearchFixture {
    home: TempDir,
    path: OsString,
    exec_ids: Vec<String>,
}

impl SearchFixture {
    async fn new() -> Result<Self> {
        let home = TempDir::new()?;
        ws::create_config_toml(home.path(), "http://127.0.0.1:9", "never")?;
        let sqlite = codex_state::SqliteConfig::new_for_testing(home.path().abs());
        let db = codex_state::StateRuntime::init(sqlite, "mock_provider".to_string()).await?;
        let mut exec_ids = Vec::new();
        let mut paths = Vec::new();
        for (hour, source) in [
            (10, SessionSource::Exec),
            (11, SessionSource::Cli),
            (12, SessionSource::Exec),
            (13, SessionSource::Exec),
        ] {
            let filename_ts = format!("2025-02-01T{hour:02}-00-00");
            let timestamp = format!("2025-02-01T{hour:02}:00:00Z");
            let id = create_fake_rollout_with_source(
                home.path(),
                &filename_ts,
                &timestamp,
                "visible needle message",
                Some("mock_provider"),
                /*git_info*/ None,
                source.clone(),
            )?;
            let path = rollout_path(home.path(), &filename_ts, &id);
            let created_at = DateTime::parse_from_rfc3339(&timestamp)?.with_timezone(&Utc);
            let mut metadata = codex_state::ThreadMetadataBuilder::new(
                ThreadId::from_string(&id)?,
                path.clone(),
                created_at,
                source.clone(),
            )
            .with_model_provider(Some("mock_provider".to_string()))
            .with_cwd(home.path().to_path_buf())
            .with_cli_version("0.0.0".to_string())
            .build("mock_provider");
            metadata.first_user_message = Some("visible needle message".to_string());
            db.upsert_thread(&metadata).await?;
            if matches!(source, SessionSource::Exec) {
                exec_ids.push(id);
            }
            paths.push(path);
        }
        db.mark_backfill_complete(/*last_watermark*/ None).await?;
        let root = home.path().join("sessions");
        let candidates = paths
            .iter()
            .map(|path| format!("{}\0{}\n", path.display(), 1))
            .collect::<String>();
        std::fs::write(root.join("rg.candidates"), candidates)?;
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin)?;
        let rg = bin.join("rg");
        codex_utils_cargo_bin::write_executable(
            &rg,
            r#"#!/bin/sh
for root do :; done
printf '%s\n' "$$" > "$root/rg.pid"
cat "$root/rg.candidates"
while ! test -f "$root/rg.release"; do sleep 0.02; done
"#,
        )?;
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(
            std::iter::once(bin).chain(std::env::split_paths(&inherited_path)),
        )?;
        Ok(Self {
            home,
            path,
            exec_ids,
        })
    }

    fn root(&self) -> PathBuf {
        self.home.path().join("sessions")
    }

    async fn spawn(&self) -> Result<(tokio::process::Child, std::net::SocketAddr)> {
        ws::spawn_websocket_server_with_env(
            self.home.path(),
            "ws://127.0.0.1:0",
            &[],
            &[("PATH", self.path.as_os_str())],
        )
        .await
    }
}

async fn initialize(stream: &mut ws::WsClient, id: i64) -> Result<()> {
    ws::send_request(
        stream,
        "initialize",
        id,
        Some(json!({
            "clientInfo": {"name": "search-test", "version": "0.1.0"},
            "capabilities": {"experimentalApi": true}
        })),
    )
    .await?;
    ws::read_response_for_id(stream, id).await?;
    Ok(())
}

fn params() -> serde_json::Value {
    json!({
        "searchTerm": "needle",
        "sourceKinds": ["exec"],
        "sortKey": "created_at",
        "sortDirection": "desc",
        "limit": 2
    })
}

async fn pid(root: &Path) -> Result<String> {
    timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(pid) = tokio::fs::read_to_string(root.join("rg.pid")).await
                && let Ok(pid) = pid.trim().parse::<u32>()
                && pid > 0
            {
                return Ok::<_, anyhow::Error>(pid.to_string());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("gated ripgrep did not start")?
}

async fn assert_reaped(pid: &str) -> Result<()> {
    let proc_path = Path::new("/proc").join(pid);
    timeout(Duration::from_secs(5), async {
        while proc_path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("ripgrep was not promptly reaped")?;
    Ok(())
}

#[tokio::test]
async fn streaming_search_is_incremental_filtered_and_matches_final_response() -> Result<()> {
    let fixture = SearchFixture::new().await?;
    let (mut server, addr) = fixture.spawn().await?;
    let mut denied = ws::connect_websocket(addr).await?;
    ws::send_initialize_request(&mut denied, /*id*/ 1, "search-without-experimental").await?;
    ws::read_response_for_id(&mut denied, /*id*/ 1).await?;
    ws::send_request(
        &mut denied,
        "thread/search/stream",
        /*id*/ 2,
        Some(params()),
    )
    .await?;
    let error = ws::read_error_for_id(&mut denied, /*id*/ 2).await?;
    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "thread/search/stream requires experimentalApi capability"
    );
    ws::send_request(
        &mut denied,
        "thread/search/stream/cancel",
        /*id*/ 3,
        Some(json!({"requestId": 2})),
    )
    .await?;
    let error = ws::read_error_for_id(&mut denied, /*id*/ 3).await?;
    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "thread/search/stream/cancel requires experimentalApi capability"
    );
    let mut client = ws::connect_websocket(addr).await?;
    initialize(&mut client, /*id*/ 1).await?;
    let mut oversized = params();
    oversized["searchTerm"] = json!("é".repeat(32_769));
    ws::send_request(
        &mut client,
        "thread/search/stream",
        /*id*/ 90,
        Some(oversized),
    )
    .await?;
    assert_eq!(
        ws::read_error_for_id(&mut client, /*id*/ 90)
            .await?
            .error
            .code,
        -32602
    );
    let mut expensive = params();
    expensive["searchTerm"] = json!("s".repeat(65_536));
    ws::send_request(
        &mut client,
        "thread/search/stream",
        /*id*/ 91,
        Some(expensive),
    )
    .await?;
    assert_eq!(
        ws::read_error_for_id(&mut client, /*id*/ 91)
            .await?
            .error
            .code,
        -32602
    );
    let mut streaming_params = params();
    streaming_params["sortDirection"] = json!("asc");
    ws::send_request(
        &mut client,
        "thread/search/stream",
        /*id*/ 2,
        Some(streaming_params),
    )
    .await?;
    let child_pid = pid(&fixture.root()).await?;
    let notice = ws::read_notification_for_method(&mut client, "thread/search/partial").await?;
    let partial: ThreadSearchPartialNotification =
        serde_json::from_value(notice.params.context("missing partial params")?)?;
    assert_eq!(partial.request_id, RequestId::Integer(2));
    assert!(!partial.data.is_empty());
    assert!(
        partial
            .data
            .iter()
            .all(|row| fixture.exec_ids.contains(&row.thread.id))
    );
    let mut streamed_ids = partial
        .data
        .into_iter()
        .map(|row| row.thread.id)
        .collect::<Vec<_>>();
    assert!(Path::new("/proc").join(&child_pid).exists());
    tokio::fs::write(fixture.root().join("rg.release"), []).await?;
    let streamed: ThreadSearchResponse = loop {
        match ws::read_jsonrpc_message(&mut client).await? {
            JSONRPCMessage::Notification(notice) if notice.method == "thread/search/partial" => {
                let partial: ThreadSearchPartialNotification =
                    serde_json::from_value(notice.params.context("missing partial params")?)?;
                assert_eq!(partial.request_id, RequestId::Integer(2));
                streamed_ids.extend(partial.data.into_iter().map(|row| row.thread.id));
            }
            JSONRPCMessage::Response(response) if response.id == RequestId::Integer(2) => {
                break serde_json::from_value(response.result)?;
            }
            _ => {}
        }
    };
    assert!(streamed_ids.len() <= 2);
    assert_eq!(
        streamed_ids.iter().collect::<HashSet<_>>().len(),
        streamed_ids.len()
    );
    assert!(streamed_ids.iter().all(|id| fixture.exec_ids.contains(id)));
    assert_eq!(
        streamed
            .data
            .iter()
            .map(|row| row.thread.id.clone())
            .collect::<Vec<_>>(),
        fixture.exec_ids.iter().take(2).cloned().collect::<Vec<_>>()
    );
    assert!(streamed.next_cursor.is_some());
    let mut plain_params = params();
    plain_params["sortDirection"] = json!("asc");
    ws::send_request(
        &mut client,
        "thread/search",
        /*id*/ 3,
        Some(plain_params),
    )
    .await?;
    let plain: ThreadSearchResponse = loop {
        match ws::read_jsonrpc_message(&mut client).await? {
            JSONRPCMessage::Notification(notice) if notice.method == "thread/search/partial" => {
                anyhow::bail!("thread/search unexpectedly emitted partial results");
            }
            JSONRPCMessage::Response(response) if response.id == RequestId::Integer(3) => {
                break serde_json::from_value(response.result)?;
            }
            _ => {}
        }
    };
    assert_eq!(streamed, plain);
    server.kill().await?;
    Ok(())
}

#[tokio::test]
async fn search_cancel_and_disconnect_stop_only_the_owning_search() -> Result<()> {
    let fixture = SearchFixture::new().await?;
    let (mut server, addr) = fixture.spawn().await?;
    let mut owner = ws::connect_websocket(addr).await?;
    let mut other = ws::connect_websocket(addr).await?;
    initialize(&mut owner, /*id*/ 1).await?;
    initialize(&mut other, /*id*/ 1).await?;
    for (method, wrong_cancel, cancel, search_id, wrong_id, cancel_id) in [
        (
            "thread/search/stream",
            "thread/search/cancel",
            "thread/search/stream/cancel",
            7,
            8,
            10,
        ),
        (
            "thread/search",
            "thread/search/stream/cancel",
            "thread/search/cancel",
            9,
            11,
            12,
        ),
    ] {
        ws::send_request(&mut owner, method, search_id, Some(params())).await?;
        let child = pid(&fixture.root()).await?;
        if search_id == 7 {
            ws::send_request(
                &mut other,
                cancel,
                /*id*/ 8,
                Some(json!({"requestId": search_id})),
            )
            .await?;
            ws::read_response_for_id(&mut other, /*id*/ 8).await?;
            assert!(Path::new("/proc").join(&child).exists());
        }
        ws::send_request(
            &mut owner,
            wrong_cancel,
            wrong_id,
            Some(json!({"requestId": search_id})),
        )
        .await?;
        ws::read_response_for_id(&mut owner, wrong_id).await?;
        assert!(Path::new("/proc").join(&child).exists());
        ws::send_request(
            &mut owner,
            cancel,
            cancel_id,
            Some(json!({"requestId": search_id})),
        )
        .await?;
        let (mut acknowledged, mut cancelled) = (false, false);
        while !acknowledged || !cancelled {
            match ws::read_jsonrpc_message(&mut owner).await? {
                JSONRPCMessage::Response(response)
                    if response.id == RequestId::Integer(cancel_id) =>
                {
                    acknowledged = true
                }
                JSONRPCMessage::Error(error) if error.id == RequestId::Integer(search_id) => {
                    assert!(error.error.message.contains("cancelled"));
                    cancelled = true;
                }
                _ => {}
            }
        }
        assert_reaped(&child).await?;
        tokio::fs::remove_file(fixture.root().join("rg.pid")).await?;
    }
    ws::send_request(
        &mut owner,
        "thread/search/stream",
        /*id*/ 13,
        Some(params()),
    )
    .await?;
    let third_pid = pid(&fixture.root()).await?;
    owner.close(None).await?;
    assert_reaped(&third_pid).await?;
    ws::send_request(
        &mut other,
        "thread/search/stream/cancel",
        /*id*/ 10,
        Some(json!({"requestId": 13})),
    )
    .await?;
    ws::read_response_for_id(&mut other, /*id*/ 10).await?;
    server.kill().await?;
    Ok(())
}
