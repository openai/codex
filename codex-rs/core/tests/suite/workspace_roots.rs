use anyhow::Context;
use anyhow::Result;
use codex_exec_server::RemoveOptions;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_path_uri::PathUri;
use core_test_support::TestTargetOs;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::test_target_os;
use serde_json::json;
use wiremock::MockServer;

const PATCH_CALL_ID: &str = "workspace-root-patch";
const COMMAND_CALL_ID: &str = "workspace-root-command";

fn workspace_roots_profile() -> PermissionProfile {
    PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
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

fn command_arguments(path: &str, contents: &str) -> Result<String> {
    let (shell, command) = match test_target_os() {
        TestTargetOs::Linux | TestTargetOs::MacOs => {
            ("bash", format!("printf %s '{contents}' > '{path}'"))
        }
        TestTargetOs::Windows => (
            "powershell",
            format!("Set-Content -NoNewline -Path '{path}' -Value '{contents}'"),
        ),
    };
    Ok(serde_json::to_string(&json!({
        "cmd": command,
        "shell": shell,
        "login": false,
    }))?)
}

async fn mount_patch_and_command_calls(
    server: &MockServer,
    patch: &str,
    command_path: &str,
    command_contents: &str,
) -> Result<ResponseMock> {
    let command_arguments = command_arguments(command_path, command_contents)?;
    Ok(mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(PATCH_CALL_ID, patch),
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
    test.submit_turn_with_permission_profile(prompt, workspace_roots_profile())
        .await
}

async fn read_file(test: &TestCodex, path: &PathUri) -> Result<String> {
    Ok(String::from_utf8(
        test.fs().read_file(path, /*sandbox*/ None).await?,
    )?)
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
async fn workspace_roots_allow_file_and_command_writes() -> Result<()> {
    const PATCH_CONTENTS: &str = "workspace root patch access";
    const COMMAND_CONTENTS: &str = "workspace root command access";

    skip_if_target_windows!(
        Ok(()),
        "sandboxed process launch is not supported by the exec-server Windows backend"
    );

    let server = start_mock_server().await;
    let test = workspace_roots_test(&server).await?;
    let cwd = PathUri::from_abs_path(&test.config.cwd);
    let patch_path = cwd.join("workspace-root-patch.txt")?;
    let command_path = cwd.join("workspace-root-command.txt")?;
    let patch = format!(
        "*** Begin Patch\n*** Add File: workspace-root-patch.txt\n+{PATCH_CONTENTS}\n*** End Patch\n"
    );

    let response_mock = mount_patch_and_command_calls(
        &server,
        &patch,
        "workspace-root-command.txt",
        COMMAND_CONTENTS,
    )
    .await?;
    submit_workspace_turn(&test, "write files inside the workspace roots").await?;

    let request = response_mock
        .last_request()
        .context("model should receive both workspace-root tool results")?;
    let (_, patch_success) = request
        .custom_tool_call_output_content_and_success(PATCH_CALL_ID)
        .context("patch result should be present")?;
    assert_ne!(patch_success, Some(false));

    let (_, command_success) = request
        .function_call_output_content_and_success(COMMAND_CALL_ID)
        .context("command result should be present")?;
    assert_ne!(command_success, Some(false));
    assert_eq!(
        read_file(&test, &patch_path).await?,
        format!("{PATCH_CONTENTS}\n")
    );
    assert_eq!(read_file(&test, &command_path).await?, COMMAND_CONTENTS);

    remove_files(&test, &[&patch_path, &command_path]).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_roots_deny_file_and_command_writes_outside_roots() -> Result<()> {
    const PATCH_CONTENTS: &str = "outside workspace root patch";
    const COMMAND_CONTENTS: &str = "outside workspace root command";

    skip_if_target_windows!(
        Ok(()),
        "sandboxed process launch is not supported by the exec-server Windows backend"
    );

    let server = start_mock_server().await;
    let test = workspace_roots_test(&server).await?;
    let patch_path = outside_workspace_path(&test, "outside-patch.txt")?;
    let command_path = outside_workspace_path(&test, "outside-command.txt")?;
    let patch_path_display = patch_path.inferred_native_path_string();
    let command_path_display = command_path.inferred_native_path_string();
    let patch = format!(
        "*** Begin Patch\n*** Add File: {patch_path_display}\n+{PATCH_CONTENTS}\n*** End Patch\n"
    );

    let response_mock =
        mount_patch_and_command_calls(&server, &patch, &command_path_display, COMMAND_CONTENTS)
            .await?;
    submit_workspace_turn(&test, "try to write files outside the workspace roots").await?;

    let request = response_mock
        .last_request()
        .context("model should receive both denied tool results")?;
    let (patch_output, patch_success) = request
        .custom_tool_call_output_content_and_success(PATCH_CALL_ID)
        .context("denied patch result should be present")?;
    assert_ne!(patch_success, Some(true));
    assert!(
        patch_output
            .as_deref()
            .is_some_and(|output| output.contains("outside of the project")),
        "patch should be denied outside the workspace roots, got {patch_output:?}"
    );

    let (command_output, _) = request
        .function_call_output_content_and_success(COMMAND_CALL_ID)
        .context("denied command result should be present")?;
    let command_output = command_output.context("denied command output should be present")?;
    assert!(
        command_output.contains(&command_path_display),
        "denied command output should identify {command_path_display}, got {command_output:?}"
    );
    assert!(
        test.fs()
            .read_file(&patch_path, /*sandbox*/ None)
            .await
            .is_err()
    );
    assert!(
        test.fs()
            .read_file(&command_path, /*sandbox*/ None)
            .await
            .is_err()
    );

    Ok(())
}
