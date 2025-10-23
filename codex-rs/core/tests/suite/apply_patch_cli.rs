#![cfg(not(target_os = "windows"))]

use anyhow::Result;
use pretty_assertions::assert_eq;
use std::fs;

use codex_core::features::Feature;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::assert_regex_match;
use core_test_support::responses::ev_apply_patch_function_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::Value;
use serde_json::json;

async fn submit_turn(test: &TestCodex, prompt: &str) -> Result<()> {
    submit_turn_with_policy(test, prompt, SandboxPolicy::DangerFullAccess).await
}

async fn submit_turn_with_policy(
    test: &TestCodex,
    prompt: &str,
    sandbox_policy: SandboxPolicy,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: prompt.into(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TaskComplete(_))
    })
    .await;
    Ok(())
}

fn function_call_output<'a>(bodies: &'a [Value], call_id: &str) -> &'a Value {
    for body in bodies {
        if let Some(items) = body.get("input").and_then(Value::as_array) {
            for item in items {
                if item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
                {
                    return item;
                }
            }
        }
    }
    panic!("function_call_output {call_id} not found");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_multiple_operations_integration() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
        config.model = "gpt-5".to_string();
        config.model_family = find_family_for_model("gpt-5").expect("gpt-5 is valid");
    });
    let test = builder.build(&server).await?;

    // Seed workspace state
    let modify_path = test.cwd.path().join("modify.txt");
    let delete_path = test.cwd.path().join("delete.txt");
    fs::write(&modify_path, "line1\nline2\n")?;
    fs::write(&delete_path, "obsolete\n")?;

    let patch = "*** Begin Patch\n*** Add File: nested/new.txt\n+created\n*** Delete File: delete.txt\n*** Update File: modify.txt\n@@\n-line2\n+changed\n*** End Patch";

    let call_id = "apply-multi-ops";
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    submit_turn(&test, "please apply multi-ops patch").await?;

    let requests = server.received_requests().await.expect("requests");
    let bodies: Vec<Value> = requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let out = function_call_output(&bodies, call_id)
        .get("output")
        .and_then(Value::as_str)
        .expect("output string");

    let expected = r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
A nested/new.txt
M modify.txt
D delete.txt
?$";
    assert_regex_match(expected, out);

    assert_eq!(
        fs::read_to_string(test.cwd.path().join("nested/new.txt"))?,
        "created\n"
    );
    assert_eq!(fs::read_to_string(&modify_path)?, "line1\nchanged\n");
    assert!(!delete_path.exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_multiple_chunks() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let target = test.cwd.path().join("multi.txt");
    fs::write(&target, "line1\nline2\nline3\nline4\n")?;

    let patch = "*** Begin Patch\n*** Update File: multi.txt\n@@\n-line2\n+changed2\n@@\n-line4\n+changed4\n*** End Patch";
    let call_id = "apply-multi-chunks";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply multi-chunk patch").await?;

    assert_eq!(
        fs::read_to_string(&target)?,
        "line1\nchanged2\nline3\nchanged4\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_moves_file_to_new_directory() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let original = test.cwd.path().join("old/name.txt");
    let new_path = test.cwd.path().join("renamed/dir/name.txt");
    fs::create_dir_all(original.parent().expect("parent"))?;
    fs::write(&original, "old content\n")?;

    let patch = "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/dir/name.txt\n@@\n-old content\n+new content\n*** End Patch";
    let call_id = "apply-move";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply move patch").await?;

    assert!(!original.exists());
    assert_eq!(fs::read_to_string(&new_path)?, "new content\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_updates_file_appends_trailing_newline() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let target = test.cwd.path().join("no_newline.txt");
    fs::write(&target, "no newline at end")?;

    let patch = "*** Begin Patch\n*** Update File: no_newline.txt\n@@\n-no newline at end\n+first line\n+second line\n*** End Patch";
    let call_id = "apply-append-nl";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply newline patch").await?;

    let contents = fs::read_to_string(&target)?;
    assert!(contents.ends_with('\n'));
    assert_eq!(contents, "first line\nsecond line\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_move_overwrites_existing_destination() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let original = test.cwd.path().join("old/name.txt");
    let destination = test.cwd.path().join("renamed/dir/name.txt");
    fs::create_dir_all(original.parent().expect("parent"))?;
    fs::create_dir_all(destination.parent().expect("parent"))?;
    fs::write(&original, "from\n")?;
    fs::write(&destination, "existing\n")?;

    let patch = "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/dir/name.txt\n@@\n-from\n+new\n*** End Patch";
    let call_id = "apply-move-overwrite";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply move overwrite patch").await?;

    assert!(!original.exists());
    assert_eq!(fs::read_to_string(&destination)?, "new\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_add_overwrites_existing_file() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let path = test.cwd.path().join("duplicate.txt");
    fs::write(&path, "old content\n")?;

    let patch = "*** Begin Patch\n*** Add File: duplicate.txt\n+new content\n*** End Patch";
    let call_id = "apply-add-overwrite";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply add overwrite patch").await?;

    assert_eq!(fs::read_to_string(&path)?, "new content\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_rejects_invalid_hunk_header() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let patch = "*** Begin Patch\n*** Frobnicate File: foo\n*** End Patch";
    let call_id = "apply-invalid-header";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply invalid header patch").await?;

    let requests = server.received_requests().await.expect("requests");
    let bodies: Vec<Value> = requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let out = function_call_output(&bodies, call_id)
        .get("output")
        .and_then(Value::as_str)
        .expect("output string");

    assert!(
        out.contains("apply_patch verification failed"),
        "expected verification failure message"
    );
    assert!(
        out.contains("is not a valid hunk header"),
        "expected parse diagnostics in output: {out:?}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_reports_missing_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let target = test.cwd.path().join("modify.txt");
    fs::write(&target, "line1\nline2\n")?;

    let patch =
        "*** Begin Patch\n*** Update File: modify.txt\n@@\n-missing\n+changed\n*** End Patch";
    let call_id = "apply-missing-context";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply missing context patch").await?;

    let requests = server.received_requests().await.expect("requests");
    let bodies: Vec<Value> = requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let out = function_call_output(&bodies, call_id)
        .get("output")
        .and_then(Value::as_str)
        .expect("output string");

    assert!(
        out.contains("apply_patch verification failed"),
        "expected verification failure message"
    );
    assert!(out.contains("Failed to find expected lines in"));
    assert_eq!(fs::read_to_string(&target)?, "line1\nline2\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_reports_missing_target_file() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let patch = "*** Begin Patch\n*** Update File: missing.txt\n@@\n-nope\n+better\n*** End Patch";
    let call_id = "apply-missing-file";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "fail"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "attempt to update a missing file").await?;

    let requests = server.received_requests().await.expect("requests");
    let bodies: Vec<Value> = requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let out = function_call_output(&bodies, call_id)
        .get("output")
        .and_then(Value::as_str)
        .expect("output string");
    assert!(
        out.contains("apply_patch verification failed"),
        "expected verification failure message"
    );
    assert!(
        out.contains("Failed to read file to update"),
        "expected missing file diagnostics: {out}"
    );
    assert!(
        out.contains("missing.txt"),
        "expected missing file path in diagnostics: {out}"
    );
    assert!(!test.cwd.path().join("missing.txt").exists());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_rejects_empty_patch() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let patch = "*** Begin Patch\n*** End Patch";
    let call_id = "apply-empty";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply empty patch").await?;

    let requests = server.received_requests().await.expect("requests");
    let bodies: Vec<Value> = requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let out = function_call_output(&bodies, call_id)
        .get("output")
        .and_then(Value::as_str)
        .expect("output string");
    assert!(
        out.contains("patch rejected: empty patch"),
        "expected rejection for empty patch: {out}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_delete_directory_reports_verification_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    fs::create_dir(test.cwd.path().join("dir"))?;

    let patch = "*** Begin Patch\n*** Delete File: dir\n*** End Patch";
    let call_id = "apply-delete-dir";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "delete a directory via apply_patch").await?;

    let requests = server.received_requests().await.expect("requests");
    let bodies: Vec<Value> = requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let out = function_call_output(&bodies, call_id)
        .get("output")
        .and_then(Value::as_str)
        .expect("output string");
    assert!(out.contains("apply_patch verification failed"));
    assert!(out.contains("Failed to read"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_rejects_path_traversal_outside_workspace() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let escape_path = test
        .cwd
        .path()
        .parent()
        .expect("cwd should have parent")
        .join("escape.txt");
    let _ = fs::remove_file(&escape_path);

    let patch = "*** Begin Patch\n*** Add File: ../escape.txt\n+outside\n*** End Patch";
    let call_id = "apply-path-traversal";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "fail"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    submit_turn_with_policy(
        &test,
        "attempt to escape workspace via apply_patch",
        sandbox_policy,
    )
    .await?;

    let requests = server.received_requests().await.expect("requests");
    let bodies: Vec<Value> = requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let out = function_call_output(&bodies, call_id)
        .get("output")
        .and_then(Value::as_str)
        .expect("output string");
    assert!(
        out.contains(
            "patch rejected: writing outside of the project; rejected by user approval settings"
        ),
        "expected rejection message for path traversal: {out}"
    );
    assert!(
        !escape_path.exists(),
        "path traversal should be rejected; tool output: {out}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_verification_failure_has_no_side_effects() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        // Use freeform feature set to ensure the tool path is exercised fully.
        config.features.enable(Feature::ApplyPatchFreeform);
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    // Compose a patch that would create a file, then fail verification on an update.
    let call_id = "apply-partial-no-side-effects";
    let patch = "*** Begin Patch\n*** Add File: created.txt\n+hello\n*** Update File: missing.txt\n@@\n-old\n+new\n*** End Patch";

    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "failed"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "attempt partial apply patch").await?;

    let created = test.cwd.path().join("created.txt");
    assert!(
        !created.exists(),
        "verification failure should prevent any filesystem changes"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_shell_heredoc_with_cd_updates_relative_workdir() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.model = "gpt-5".to_string();
        config.model_family = find_family_for_model("gpt-5").expect("gpt-5 is valid");
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    // Prepare a file inside a subdir; update it via cd && apply_patch heredoc form.
    let sub = test.cwd.path().join("sub");
    fs::create_dir_all(&sub)?;
    let target = sub.join("in_sub.txt");
    fs::write(&target, "before\n")?;

    let script = "cd sub && apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: in_sub.txt\n@@\n-before\n+after\n*** End Patch\nEOF\n";
    let call_id = "shell-heredoc-cd";
    let args = json!({
        "command": ["bash", "-lc", script],
        "timeout_ms": 5_000,
    });
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply via shell heredoc with cd").await?;

    let requests = server.received_requests().await.expect("requests");
    let bodies: Vec<Value> = requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let out = function_call_output(&bodies, call_id)
        .get("output")
        .and_then(Value::as_str)
        .expect("output string");
    assert!(
        out.contains("Success."),
        "expected successful apply_patch invocation via shell: {out}"
    );
    assert_eq!(fs::read_to_string(&target)?, "after\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_shell_failure_propagates_error_and_skips_diff() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.model = "gpt-5".to_string();
        config.model_family = find_family_for_model("gpt-5").expect("gpt-5 is valid");
        config.include_apply_patch_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let target = cwd.path().join("invalid.txt");
    fs::write(&target, "ok\n")?;

    let script = "apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: invalid.txt\n@@\n-nope\n+changed\n*** End Patch\nEOF\n";
    let call_id = "shell-apply-failure";
    let args = json!({
        "command": ["bash", "-lc", script],
        "timeout_ms": 5_000,
    });
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "fail"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    let model = session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "apply patch via shell".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let mut saw_turn_diff = false;
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnDiff(_) => {
            saw_turn_diff = true;
            false
        }
        EventMsg::TaskComplete(_) => true,
        _ => false,
    })
    .await;

    assert!(
        !saw_turn_diff,
        "turn diff should not be emitted when shell apply_patch fails verification"
    );

    let requests = server.received_requests().await.expect("requests");
    let bodies: Vec<Value> = requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let out = function_call_output(&bodies, call_id)
        .get("output")
        .and_then(Value::as_str)
        .expect("output string");
    assert!(
        out.contains("apply_patch verification failed"),
        "expected verification failure message"
    );
    assert!(
        out.contains("Failed to find expected lines in"),
        "expected failure diagnostics: {out}"
    );
    assert!(
        out.contains("invalid.txt"),
        "expected file path in output: {out}"
    );
    assert_eq!(fs::read_to_string(&target)?, "ok\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_function_accepts_lenient_heredoc_wrapped_patch() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let file_name = "lenient.txt";
    let patch_inner =
        format!("*** Begin Patch\n*** Add File: {file_name}\n+lenient\n*** End Patch\n");
    let wrapped = format!("<<'EOF'\n{patch_inner}EOF\n");
    let call_id = "apply-lenient";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, &wrapped),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply lenient heredoc patch").await?;

    let new_file = test.cwd.path().join(file_name);
    assert_eq!(fs::read_to_string(new_file)?, "lenient\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_end_of_file_anchor() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let target = test.cwd.path().join("tail.txt");
    fs::write(&target, "alpha\nlast\n")?;

    let patch = "*** Begin Patch\n*** Update File: tail.txt\n@@\n-last\n+end\n*** End of File\n*** End Patch";
    let call_id = "apply-eof";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply EOF-anchored patch").await?;
    assert_eq!(fs::read_to_string(&target)?, "alpha\nend\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_missing_second_chunk_context_rejected() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let target = test.cwd.path().join("two_chunks.txt");
    fs::write(&target, "a\nb\nc\nd\n")?;

    // First chunk has @@, second chunk intentionally omits @@ to trigger parse error.
    let patch =
        "*** Begin Patch\n*** Update File: two_chunks.txt\n@@\n-b\n+B\n\n-d\n+D\n*** End Patch";
    let call_id = "apply-missing-ctx-2nd";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "fail"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply missing context second chunk").await?;

    let requests = server.received_requests().await.expect("requests");
    let bodies: Vec<Value> = requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let out = function_call_output(&bodies, call_id)
        .get("output")
        .and_then(Value::as_str)
        .expect("output string");
    assert!(out.contains("apply_patch verification failed"));
    assert!(
        out.contains("Failed to find expected lines in"),
        "expected hunk context diagnostics: {out}"
    );
    // Original file unchanged on failure
    assert_eq!(fs::read_to_string(&target)?, "a\nb\nc\nd\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_emits_turn_diff_event_with_unified_diff() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call_id = "apply-diff-event";
    let file = "udiff.txt";
    let patch = format!("*** Begin Patch\n*** Add File: {file}\n+hello\n*** End Patch\n");
    let first = sse(vec![
        ev_response_created("resp-1"),
        ev_apply_patch_function_call(call_id, &patch),
        ev_completed("resp-1"),
    ]);
    let second = sse(vec![
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-2"),
    ]);
    mount_sse_sequence(&server, vec![first, second]).await;

    let model = session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "emit diff".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let mut saw_turn_diff = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnDiff(ev) => {
            saw_turn_diff = Some(ev.unified_diff.clone());
            false
        }
        EventMsg::TaskComplete(_) => true,
        _ => false,
    })
    .await;

    let diff = saw_turn_diff.expect("expected TurnDiff event");
    // Basic markers of a unified diff with file addition
    assert!(diff.contains("diff --git"), "diff header missing: {diff:?}");
    assert!(diff.contains("--- /dev/null") || diff.contains("--- a/"));
    assert!(diff.contains("+++ b/"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_turn_diff_for_rename_with_content_change() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    // Seed original file
    let old = cwd.path().join("old.txt");
    fs::write(&old, "old\n")?;

    // Patch: update + move
    let call_id = "apply-rename-change";
    let patch = "*** Begin Patch\n*** Update File: old.txt\n*** Move to: new.txt\n@@\n-old\n+new\n*** End Patch";
    let first = sse(vec![
        ev_response_created("resp-1"),
        ev_apply_patch_function_call(call_id, patch),
        ev_completed("resp-1"),
    ]);
    let second = sse(vec![
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-2"),
    ]);
    mount_sse_sequence(&server, vec![first, second]).await;

    let model = session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "rename with change".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let mut last_diff: Option<String> = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnDiff(ev) => {
            last_diff = Some(ev.unified_diff.clone());
            false
        }
        EventMsg::TaskComplete(_) => true,
        _ => false,
    })
    .await;

    let diff = last_diff.expect("expected TurnDiff event after rename");
    // Basic checks: shows old -> new, and the content delta
    assert!(diff.contains("old.txt"), "diff missing old path: {diff:?}");
    assert!(diff.contains("new.txt"), "diff missing new path: {diff:?}");
    assert!(diff.contains("--- a/"), "missing old header");
    assert!(diff.contains("+++ b/"), "missing new header");
    assert!(diff.contains("-old\n"), "missing removal line");
    assert!(diff.contains("+new\n"), "missing addition line");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_aggregates_diff_across_multiple_tool_calls() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;

    let call1 = "agg-1";
    let call2 = "agg-2";
    let patch1 = "*** Begin Patch\n*** Add File: agg/a.txt\n+v1\n*** End Patch";
    let patch2 = "*** Begin Patch\n*** Update File: agg/a.txt\n@@\n-v1\n+v2\n*** Add File: agg/b.txt\n+B\n*** End Patch";

    let s1 = sse(vec![
        ev_response_created("resp-1"),
        ev_apply_patch_function_call(call1, patch1),
        ev_completed("resp-1"),
    ]);
    let s2 = sse(vec![
        ev_response_created("resp-2"),
        ev_apply_patch_function_call(call2, patch2),
        ev_completed("resp-2"),
    ]);
    let s3 = sse(vec![
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-3"),
    ]);
    mount_sse_sequence(&server, vec![s1, s2, s3]).await;

    let model = session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "aggregate diffs".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let mut last_diff: Option<String> = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnDiff(ev) => {
            last_diff = Some(ev.unified_diff.clone());
            false
        }
        EventMsg::TaskComplete(_) => true,
        _ => false,
    })
    .await;

    let diff = last_diff.expect("expected TurnDiff after two patches");
    assert!(diff.contains("agg/a.txt"), "diff missing a.txt");
    assert!(diff.contains("agg/b.txt"), "diff missing b.txt");
    // Final content reflects v2 for a.txt
    assert!(diff.contains("+v2\n") || diff.contains("v2\n"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_change_context_disambiguates_target() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let target = test.cwd.path().join("multi_ctx.txt");
    fs::write(&target, "fn a\nx=10\ny=2\nfn b\nx=10\ny=20\n")?;

    let patch =
        "*** Begin Patch\n*** Update File: multi_ctx.txt\n@@ fn b\n-x=10\n+x=11\n*** End Patch";
    let call_id = "apply-ctx";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, bodies).await;

    submit_turn(&test, "apply with change_context").await?;

    let contents = fs::read_to_string(&target)?;
    assert_eq!(contents, "fn a\nx=10\ny=2\nfn b\nx=11\ny=20\n");
    Ok(())
}
