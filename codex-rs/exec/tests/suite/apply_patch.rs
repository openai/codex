#![allow(clippy::unwrap_used, unused_imports)]

use anyhow::Context;
use assert_cmd::prelude::*;
use codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_completed;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

/// While we may add an `apply-patch` subcommand to the `codex` CLI multitool
/// at some point, we must ensure that the smaller `codex-exec` CLI can still
/// emulate the `apply_patch` CLI.
#[test]
fn test_standalone_exec_cli_can_use_apply_patch() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let relative_path = "source.txt";
    let absolute_path = tmp.path().join(relative_path);
    fs::write(&absolute_path, "original content\n")?;

    Command::new(codex_utils_cargo_bin::cargo_bin("codex-exec")?)
        .arg(CODEX_CORE_APPLY_PATCH_ARG1)
        .arg(
            r#"*** Begin Patch
*** Update File: source.txt
@@
-original content
+modified by apply_patch
*** End Patch"#,
        )
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout("Success. Updated the following files:\nM source.txt\n")
        .stderr(predicates::str::is_empty());
    assert_eq!(
        fs::read_to_string(absolute_path)?,
        "modified by apply_patch\n"
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_apply_patch_tool() -> anyhow::Result<()> {
    use core_test_support::skip_if_no_network;
    use core_test_support::test_codex_exec::test_codex_exec;

    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let tmp_path = test.cwd_path().to_path_buf();
    let add_patch = r#"*** Begin Patch
*** Add File: test.md
+Hello world
*** End Patch"#;
    let update_patch = r#"*** Begin Patch
*** Update File: test.md
@@
-Hello world
+Final text
*** End Patch"#;
    let response_streams = vec![
        sse(vec![
            ev_apply_patch_custom_tool_call("request_0", add_patch),
            ev_completed("request_0"),
        ]),
        sse(vec![
            ev_apply_patch_custom_tool_call("request_1", update_patch),
            ev_completed("request_1"),
        ]),
        sse(vec![ev_completed("request_2")]),
    ];
    let server = start_mock_server().await;
    mount_sse_sequence(&server, response_streams).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-s")
        .arg("danger-full-access")
        .arg("foo")
        .assert()
        .success();

    let final_path = tmp_path.join("test.md");
    let contents = std::fs::read_to_string(&final_path).expect("final file should be readable");
    assert_eq!(contents, "Final text\n");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_apply_patch_freeform_tool() -> anyhow::Result<()> {
    use core_test_support::skip_if_no_network;
    use core_test_support::test_codex_exec::test_codex_exec;

    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let freeform_add_patch = r#"*** Begin Patch
*** Add File: app.py
+class BaseClass:
+  def method():
+    return False
*** End Patch"#;
    let freeform_update_patch = r#"*** Begin Patch
*** Update File: app.py
@@  def method():
-    return False
+
+    return True
*** End Patch"#;
    let response_streams = vec![
        sse(vec![
            ev_apply_patch_custom_tool_call("request_0", freeform_add_patch),
            ev_completed("request_0"),
        ]),
        sse(vec![
            ev_apply_patch_custom_tool_call("request_1", freeform_update_patch),
            ev_completed("request_1"),
        ]),
        sse(vec![ev_completed("request_2")]),
    ];
    let server = start_mock_server().await;
    mount_sse_sequence(&server, response_streams).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-s")
        .arg("danger-full-access")
        .arg("foo")
        .assert()
        .success();

    // Verify final file contents
    let final_path = test.cwd_path().join("app.py");
    let contents = std::fs::read_to_string(&final_path).expect("final file should be readable");
    assert_eq!(
        contents,
        include_str!("../fixtures/apply_patch_freeform_final.txt")
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shutdown_flushes_completed_turn_and_file_diff() -> anyhow::Result<()> {
    use codex_login::AuthDotJson;
    use codex_login::AuthKeyringBackendKind;
    use codex_login::TokenData;
    use codex_login::save_auth;
    use codex_login::token_data::IdTokenInfo;
    use codex_protocol::auth::AuthMode;
    use core_test_support::skip_if_no_network;
    use core_test_support::test_codex_exec::test_codex_exec;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use wiremock::Mock;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;

    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let run_dir = test.cwd_path();
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(run_dir)
            .status()
            .context("initialize test git repository")?
            .success()
    );
    assert!(
        std::process::Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/test-org/turn-profile-test.git",
            ])
            .current_dir(run_dir)
            .status()
            .context("add test git remote")?
            .success()
    );
    save_auth(
        test.home_path(),
        &AuthDotJson {
            auth_mode: Some(AuthMode::Chatgpt),
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: IdTokenInfo {
                    raw_jwt: "header.eyJhbGciOiJub25lIn0.eyJzdWIiOiJ1c2VyLTEyMyJ9.".to_string(),
                    ..Default::default()
                },
                access_token: "test-access-token".to_string(),
                refresh_token: "test-refresh-token".to_string(),
                account_id: None,
            }),
            last_refresh: None,
            agent_identity: None,
            personal_access_token: None,
            bedrock_api_key: None,
        },
        codex_login::AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let server = start_mock_server().await;
    let deliveries = Arc::new(AtomicUsize::new(0));
    let completed_deliveries = Arc::clone(&deliveries);
    Mock::given(method("POST"))
        .and(|request: &wiremock::Request| request.url.path().ends_with("events"))
        .respond_with(move |_request: &wiremock::Request| {
            // A request is counted only once its delayed handler has completed.
            std::thread::sleep(Duration::from_millis(750));
            completed_deliveries.fetch_add(1, Ordering::Release);
            ResponseTemplate::new(200)
        })
        .expect(3..)
        .mount(&server)
        .await;
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_apply_patch_custom_tool_call(
                    "request_0",
                    "*** Begin Patch\n*** Add File: flushed.md\n+persist this line\n*** End Patch",
                ),
                ev_completed("request_0"),
            ]),
            sse(vec![ev_completed("request_1")]),
        ],
    )
    .await;

    test.cmd_with_server(&server)
        .env_remove("CODEX_API_KEY")
        .env_remove("CODEX_ANALYTICS_EVENTS_CAPTURE_FILE")
        .arg("--skip-git-repo-check")
        .arg("-s")
        .arg("danger-full-access")
        .arg("-c")
        .arg("model_provider=\"test\"")
        .arg("-c")
        .arg(format!(
            "model_providers.test={{name=\"test\",base_url={:?},wire_api=\"responses\",requires_openai_auth=false,supports_websockets=false}}",
            format!("{}/v1", server.uri())
        ))
        .arg("-c")
        .arg("analytics.enabled=true")
        .arg("-c")
        .arg(format!("chatgpt_base_url={}", server.uri()))
        .arg("write the file")
        .assert()
        .success();

    let persisted = std::fs::read_to_string(run_dir.join("flushed.md"))
        .context("read file written by apply_patch")?;
    assert_eq!(persisted, "persist this line\n");

    let requests = server
        .received_requests()
        .await
        .context("read analytics requests")?;
    let request_paths = requests
        .iter()
        .map(|request| request.url.path().to_string())
        .collect::<Vec<_>>();
    let event_types = requests
        .iter()
        .filter(|request| request.url.path().ends_with("events"))
        .flat_map(|request| {
            serde_json::from_slice::<serde_json::Value>(&request.body)
                .ok()
                .and_then(|body| {
                    body.get("events")
                        .and_then(serde_json::Value::as_array)
                        .cloned()
                })
                .unwrap_or_default()
                .into_iter()
                .filter_map(|event| event["event_type"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(
        deliveries.load(Ordering::Acquire) >= 3,
        "the runtime exited before the delayed analytics deliveries completed (completed={}, events={event_types:?}, paths={request_paths:?})",
        deliveries.load(Ordering::Acquire)
    );
    assert!(
        event_types.iter().any(|event| event == "codex_turn_event"),
        "no completed event was delivered; request paths: {request_paths:?}"
    );
    assert!(
        event_types
            .iter()
            .any(|event| event == "codex_accepted_line_fingerprints"),
        "no accepted-line event was delivered; request paths: {request_paths:?}"
    );

    Ok(())
}
