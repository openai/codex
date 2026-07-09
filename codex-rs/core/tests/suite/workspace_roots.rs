use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_exec_server::RemoveOptions;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::EventMsg;
use codex_utils_path_uri::PathUri;
use core_test_support::TestTargetOs;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_remote_env;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::test_target_os;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::time::Duration;
use wiremock::MockServer;

const IMAGE_CALL_ID: &str = "workspace-root-image";
const COMMAND_CALL_ID: &str = "workspace-root-command";
const TURN_COMPLETE_TIMEOUT: Duration = Duration::from_secs(30);
const PNG_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

fn workspace_roots_read_profile() -> PermissionProfile {
    PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Minimal,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Read,
            },
        ]),
        NetworkSandboxPolicy::Restricted,
    )
}

async fn workspace_roots_test(server: &MockServer) -> Result<TestCodex> {
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
        config.workspace_roots = vec![config.cwd.clone()];
    });
    builder.build_with_auto_env(server).await
}

fn outside_workspace_path(test: &TestCodex, file_name: &str) -> Result<PathUri> {
    let file_name = format!("codex-workspace-roots-{}-{file_name}", std::process::id());
    PathUri::from_abs_path(&test.config.cwd)
        .parent()
        .context("test workspace should have a parent")?
        .join(&file_name)
        .map_err(Into::into)
}

fn command_arguments(path: &str) -> Result<String> {
    let (shell, command) = match test_target_os() {
        TestTargetOs::Linux => ("bash", format!("cat '{path}'")),
        TestTargetOs::Windows => ("powershell", format!("Get-Content -Raw '{path}'")),
        TestTargetOs::MacOs => unreachable!("remote test targets do not run macOS"),
    };
    Ok(serde_json::to_string(&json!({
        "cmd": command,
        "shell": shell,
        "login": false,
        "yield_time_ms": 10_000,
    }))?)
}

async fn mount_file_and_command_calls(
    server: &MockServer,
    image_path: &str,
    command_path: &str,
) -> Result<ResponseMock> {
    let command_arguments = command_arguments(command_path)?;
    Ok(mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    IMAGE_CALL_ID,
                    "view_image",
                    &json!({ "path": image_path }).to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call(COMMAND_CALL_ID, "exec_command", &command_arguments),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await)
}

async fn submit_workspace_turn(test: &TestCodex, prompt: &str) -> Result<()> {
    test.submit_turn_with_permission_profile(prompt, workspace_roots_read_profile())
        .await?;
    wait_for_event_with_timeout(
        &test.codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        TURN_COMPLETE_TIMEOUT,
    )
    .await;
    Ok(())
}

async fn remove_files(test: &TestCodex, paths: &[&PathUri]) -> Result<()> {
    for path in paths {
        test.fs()
            .remove(
                path,
                RemoveOptions {
                    recursive: false,
                    force: true,
                },
                /*sandbox*/ None,
            )
            .await?;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_workspace_roots_allow_file_read_and_command_run() -> Result<()> {
    const COMMAND_CONTENTS: &str = "workspace root command access";

    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let test = workspace_roots_test(&server).await?;
    let cwd = PathUri::from_abs_path(&test.config.cwd);
    let image_path = cwd.join("workspace-root.png")?;
    let text_path = cwd.join("workspace-root.txt")?;
    test.fs()
        .write_file(
            &image_path,
            BASE64_STANDARD.decode(PNG_BASE64)?,
            /*sandbox*/ None,
        )
        .await?;
    test.fs()
        .write_file(
            &text_path,
            COMMAND_CONTENTS.as_bytes().to_vec(),
            /*sandbox*/ None,
        )
        .await?;

    let response_mock =
        mount_file_and_command_calls(&server, "workspace-root.png", "workspace-root.txt").await?;
    submit_workspace_turn(&test, "read files inside the workspace roots").await?;

    let request = response_mock
        .last_request()
        .context("model should receive both workspace-root tool results")?;
    let image_output = request.function_call_output(IMAGE_CALL_ID);
    let image_url = image_output
        .get("output")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("image_url"))
        .and_then(Value::as_str)
        .context("remote filesystem read should return an image")?;
    assert!(image_url.starts_with("data:image/png;base64,"));

    let (command_output, success) = request
        .function_call_output_content_and_success(COMMAND_CALL_ID)
        .context("remote command result should be present")?;
    assert_ne!(success, Some(false));
    assert!(command_output.is_some_and(|output| output.contains(COMMAND_CONTENTS)));

    remove_files(&test, &[&image_path, &text_path]).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_workspace_roots_deny_file_and_command_reads_outside_roots() -> Result<()> {
    const OUTSIDE_CONTENTS: &str = "outside workspace root";

    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let test = workspace_roots_test(&server).await?;
    let image_path = outside_workspace_path(&test, "outside.png")?;
    let text_path = outside_workspace_path(&test, "outside.txt")?;
    test.fs()
        .write_file(
            &image_path,
            BASE64_STANDARD.decode(PNG_BASE64)?,
            /*sandbox*/ None,
        )
        .await?;
    test.fs()
        .write_file(
            &text_path,
            OUTSIDE_CONTENTS.as_bytes().to_vec(),
            /*sandbox*/ None,
        )
        .await?;
    let image_path_display = image_path.inferred_native_path_string();
    let text_path_display = text_path.inferred_native_path_string();

    let response_mock =
        mount_file_and_command_calls(&server, &image_path_display, &text_path_display).await?;
    submit_workspace_turn(&test, "try to read files outside the workspace roots").await?;

    let request = response_mock
        .last_request()
        .context("model should receive both denied tool results")?;
    let (file_output, file_success) = request
        .function_call_output_content_and_success(IMAGE_CALL_ID)
        .context("denied remote file-read result should be present")?;
    assert_eq!(file_success, Some(false));
    assert!(file_output.is_some_and(|output| {
        output.starts_with(&format!(
            "unable to locate image at `{image_path_display}`:"
        )) || output.starts_with(&format!("unable to read image at `{image_path_display}`:"))
    }));

    let (command_output, command_success) = request
        .function_call_output_content_and_success(COMMAND_CALL_ID)
        .context("denied remote command result should be present")?;
    assert_eq!(command_success, Some(false));
    assert!(command_output.is_none_or(|output| !output.contains(OUTSIDE_CONTENTS)));

    remove_files(&test, &[&image_path, &text_path]).await
}
