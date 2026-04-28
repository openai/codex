#![allow(clippy::expect_used)]

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use core_test_support::responses::ev_apply_patch_call;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_shell_command_call;
use core_test_support::test_codex::ApplyPatchModelOutput;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
#[cfg(target_os = "linux")]
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;
use core_test_support::assert_regex_match;
use core_test_support::responses::ev_apply_patch_function_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_shell_command_call_with_args;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use serde_json::json;
use test_case::test_case;
use wiremock::Mock;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

pub async fn apply_patch_harness() -> Result<TestCodexHarness> {
    apply_patch_harness_with(|builder| builder).await
}

async fn apply_patch_harness_with(
    configure: impl FnOnce(TestCodexBuilder) -> TestCodexBuilder,
) -> Result<TestCodexHarness> {
    let builder = configure(test_codex()).with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    // Box harness construction so apply_patch_cli tests do not inline the
    // full test-thread startup path into each test future.
    Box::pin(TestCodexHarness::with_remote_aware_builder(builder)).await
}

pub async fn mount_apply_patch(
    harness: &TestCodexHarness,
    call_id: &str,
    patch: &str,
    assistant_msg: &str,
    output_type: ApplyPatchModelOutput,
) {
    mount_sse_sequence(
        harness.server(),
        apply_patch_responses(call_id, patch, assistant_msg, output_type),
    )
    .await;
}

fn apply_patch_responses(
    call_id: &str,
    patch: &str,
    assistant_msg: &str,
    output_type: ApplyPatchModelOutput,
) -> Vec<String> {
    vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_call(call_id, patch, output_type),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", assistant_msg),
            ev_completed("resp-2"),
        ]),
    ]
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_uses_codex_self_exe_with_linux_sandbox_helper_alias() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;
    let codex_linux_sandbox_exe = harness
        .test()
        .config
        .codex_linux_sandbox_exe
        .as_ref()
        .expect("linux test config should include codex-linux-sandbox helper");
    assert_eq!(
        codex_linux_sandbox_exe
            .file_name()
            .and_then(|name| name.to_str()),
        Some(CODEX_LINUX_SANDBOX_ARG0),
    );

    let patch = "*** Begin Patch\n*** Add File: helper-alias.txt\n+hello\n*** End Patch";
    let call_id = "apply-helper-alias";
    mount_apply_patch(
        &harness,
        call_id,
        patch,
        "done",
        ApplyPatchModelOutput::Function,
    )
    .await;

    harness.submit("please apply helper alias patch").await?;

    let out = harness
        .apply_patch_output(call_id, ApplyPatchModelOutput::Function)
        .await;
    assert_regex_match(
        r"(?s)^Exit code: 0.*Success\. Updated the following files:\nA helper-alias\.txt\n?$",
        &out,
    );
    assert_eq!(harness.read_file_text("helper-alias.txt").await?, "hello\n");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
async fn apply_patch_cli_multiple_operations_integration(
    output_type: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness_with(|builder| builder.with_model("gpt-5.4")).await?;

    // Seed workspace state
    harness.write_file("modify.txt", "line1\nline2\n").await?;
    harness.write_file("delete.txt", "obsolete\n").await?;

    let patch = "*** Begin Patch\n*** Add File: nested/new.txt\n+created\n*** Delete File: delete.txt\n*** Update File: modify.txt\n@@\n-line2\n+changed\n*** End Patch";

    let call_id = "apply-multi-ops";
    mount_apply_patch(&harness, call_id, patch, "done", output_type).await;

    harness.submit("please apply multi-ops patch").await?;

    let out = harness.apply_patch_output(call_id, output_type).await;

    let expected = r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
A nested/new.txt
M modify.txt
D delete.txt
?$";
    assert_regex_match(expected, &out);

    assert_eq!(harness.read_file_text("nested/new.txt").await?, "created\n");
    assert_eq!(
        harness.read_file_text("modify.txt").await?,
        "line1\nchanged\n"
    );
    assert!(!harness.path_exists("delete.txt").await?);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_multiple_chunks(model_output: ApplyPatchModelOutput) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness
        .write_file("multi.txt", "line1\nline2\nline3\nline4\n")
        .await?;

    let patch = "*** Begin Patch\n*** Update File: multi.txt\n@@\n-line2\n+changed2\n@@\n-line4\n+changed4\n*** End Patch";
    let call_id = "apply-multi-chunks";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("apply multi-chunk patch").await?;

    assert_eq!(
        harness.read_file_text("multi.txt").await?,
        "line1\nchanged2\nline3\nchanged4\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_moves_file_to_new_directory(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness.write_file("old/name.txt", "old content\n").await?;

    let patch = "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/dir/name.txt\n@@\n-old content\n+new content\n*** End Patch";
    let call_id = "apply-move";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("apply move patch").await?;

    assert!(!harness.path_exists("old/name.txt").await?);
    assert_eq!(
        harness.read_file_text("renamed/dir/name.txt").await?,
        "new content\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_updates_file_appends_trailing_newline(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness
        .write_file("no_newline.txt", "no newline at end")
        .await?;

    let patch = "*** Begin Patch\n*** Update File: no_newline.txt\n@@\n-no newline at end\n+first line\n+second line\n*** End Patch";
    let call_id = "apply-append-nl";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("apply newline patch").await?;

    let contents = harness.read_file_text("no_newline.txt").await?;
    assert!(contents.ends_with('\n'));
    assert_eq!(contents, "first line\nsecond line\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_insert_only_hunk_modifies_file(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness
        .write_file("insert_only.txt", "alpha\nomega\n")
        .await?;

    let patch = "*** Begin Patch\n*** Update File: insert_only.txt\n@@\n alpha\n+beta\n omega\n*** End Patch";
    let call_id = "apply-insert-only";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("insert lines via apply_patch").await?;

    assert_eq!(
        harness.read_file_text("insert_only.txt").await?,
        "alpha\nbeta\nomega\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_move_overwrites_existing_destination(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness.write_file("old/name.txt", "from\n").await?;
    harness
        .write_file("renamed/dir/name.txt", "existing\n")
        .await?;

    let patch = "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/dir/name.txt\n@@\n-from\n+new\n*** End Patch";
    let call_id = "apply-move-overwrite";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("apply move overwrite patch").await?;

    assert!(!harness.path_exists("old/name.txt").await?);
    assert_eq!(
        harness.read_file_text("renamed/dir/name.txt").await?,
        "new\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_move_without_content_change_has_no_turn_diff(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "TurnDiffTracker currently reads the test-runner filesystem, not the remote executor filesystem",
    );

    let harness = apply_patch_harness().await?;
    let test = harness.test();
    let codex = test.codex.clone();

    harness.write_file("old/name.txt", "same\n").await?;

    let patch = "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/name.txt\n@@\n same\n*** End Patch";
    let call_id = "apply-move-no-change";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    let model = test.session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            environments: None,
            items: vec![UserInput::Text {
                text: "rename without content change".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: harness.cwd().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            permission_profile: None,
            model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut saw_turn_diff = false;
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnDiff(_) => {
            saw_turn_diff = true;
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    assert!(!saw_turn_diff, "pure rename should not emit a turn diff");
    assert!(!harness.path_exists("old/name.txt").await?);
    assert_eq!(harness.read_file_text("renamed/name.txt").await?, "same\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_add_overwrites_existing_file(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness.write_file("duplicate.txt", "old content\n").await?;

    let patch = "*** Begin Patch\n*** Add File: duplicate.txt\n+new content\n*** End Patch";
    let call_id = "apply-add-overwrite";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("apply add overwrite patch").await?;

    assert_eq!(
        harness.read_file_text("duplicate.txt").await?,
        "new content\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_rejects_invalid_hunk_header(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let patch = "*** Begin Patch\n*** Frobnicate File: foo\n*** End Patch";
    let call_id = "apply-invalid-header";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("apply invalid header patch").await?;

    let out = harness.apply_patch_output(call_id, model_output).await;

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
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_reports_missing_context(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness.write_file("modify.txt", "line1\nline2\n").await?;

    let patch =
        "*** Begin Patch\n*** Update File: modify.txt\n@@\n-missing\n+changed\n*** End Patch";
    let call_id = "apply-missing-context";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("apply missing context patch").await?;

    let out = harness.apply_patch_output(call_id, model_output).await;

    assert!(
        out.contains("apply_patch verification failed"),
        "expected verification failure message"
    );
    assert!(out.contains("Failed to find expected lines in"));
    assert_eq!(
        harness.read_file_text("modify.txt").await?,
        "line1\nline2\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_reports_missing_target_file(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let patch = "*** Begin Patch\n*** Update File: missing.txt\n@@\n-nope\n+better\n*** End Patch";
    let call_id = "apply-missing-file";
    mount_apply_patch(&harness, call_id, patch, "fail", model_output).await;

    harness.submit("attempt to update a missing file").await?;

    let out = harness.apply_patch_output(call_id, model_output).await;
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
    assert!(!harness.path_exists("missing.txt").await?);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_delete_missing_file_reports_error(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let patch = "*** Begin Patch\n*** Delete File: missing.txt\n*** End Patch";
    let call_id = "apply-delete-missing";
    mount_apply_patch(&harness, call_id, patch, "fail", model_output).await;

    harness.submit("attempt to delete missing file").await?;

    let out = harness.apply_patch_output(call_id, model_output).await;

    assert!(
        out.contains("apply_patch verification failed"),
        "expected verification failure message: {out}"
    );
    assert!(
        out.contains("Failed to read"),
        "missing delete diagnostics should mention read failure: {out}"
    );
    assert!(
        out.contains("missing.txt"),
        "missing delete diagnostics should surface target path: {out}"
    );
    assert!(!harness.path_exists("missing.txt").await?);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_rejects_empty_patch(model_output: ApplyPatchModelOutput) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let patch = "*** Begin Patch\n*** End Patch";
    let call_id = "apply-empty";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("apply empty patch").await?;

    let out = harness.apply_patch_output(call_id, model_output).await;
    assert!(
        out.contains("patch rejected: empty patch"),
        "expected rejection for empty patch: {out}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_delete_directory_reports_verification_error(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness.create_dir_all("dir").await?;

    let patch = "*** Begin Patch\n*** Delete File: dir\n*** End Patch";
    let call_id = "apply-delete-dir";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("delete a directory via apply_patch").await?;

    let out = harness.apply_patch_output(call_id, model_output).await;
    assert!(out.contains("apply_patch verification failed"));
    assert!(out.contains("Failed to read"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_rejects_path_traversal_outside_workspace(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let escape_path = harness
        .test()
        .config
        .cwd
        .parent()
        .expect("cwd should have parent")
        .join("escape.txt");
    harness.remove_abs_path(&escape_path).await?;

    let patch = "*** Begin Patch\n*** Add File: ../escape.txt\n+outside\n*** End Patch";
    let call_id = "apply-path-traversal";
    mount_apply_patch(&harness, call_id, patch, "fail", model_output).await;

    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    harness
        .submit_with_policy(
            "attempt to escape workspace via apply_patch",
            sandbox_policy,
        )
        .await?;

    let out = harness.apply_patch_output(call_id, model_output).await;
    assert!(
        out.contains(
            "patch rejected: writing outside of the project; rejected by user approval settings"
        ),
        "expected rejection message for path traversal: {out}"
    );
    assert!(
        !harness.abs_path_exists(&escape_path).await?,
        "path traversal should be rejected; tool output: {out}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_rejects_move_path_traversal_outside_workspace(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let escape_path = harness
        .test()
        .config
        .cwd
        .parent()
        .expect("cwd should have parent")
        .join("escape-move.txt");
    harness.remove_abs_path(&escape_path).await?;

    harness.write_file("stay.txt", "from\n").await?;

    let patch = "*** Begin Patch\n*** Update File: stay.txt\n*** Move to: ../escape-move.txt\n@@\n-from\n+to\n*** End Patch";
    let call_id = "apply-move-traversal";
    mount_apply_patch(&harness, call_id, patch, "fail", model_output).await;

    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    harness
        .submit_with_policy("attempt move traversal via apply_patch", sandbox_policy)
        .await?;

    let out = harness.apply_patch_output(call_id, model_output).await;
    assert!(
        out.contains(
            "patch rejected: writing outside of the project; rejected by user approval settings"
        ),
        "expected rejection message for path traversal: {out}"
    );
    assert!(
        !harness.abs_path_exists(&escape_path).await?,
        "move path traversal should be rejected; tool output: {out}"
    );
    assert_eq!(harness.read_file_text("stay.txt").await?, "from\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_verification_failure_has_no_side_effects(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness_with(|builder| {
        builder.with_config(|config| {
            config
                .features
                .enable(Feature::ApplyPatchFreeform)
                .expect("test config should allow feature update");
        })
    })
    .await?;

    // Compose a patch that would create a file, then fail verification on an update.
    let call_id = "apply-partial-no-side-effects";
    let patch = "*** Begin Patch\n*** Add File: created.txt\n+hello\n*** Update File: missing.txt\n@@\n-old\n+new\n*** End Patch";

    mount_apply_patch(&harness, call_id, patch, "failed", model_output).await;

    harness.submit("attempt partial apply patch").await?;

    assert!(
        !harness.path_exists("created.txt").await?,
        "verification failure should prevent any filesystem changes"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_shell_command_heredoc_with_cd_updates_relative_workdir() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness_with(|builder| builder.with_model("gpt-5.4")).await?;

    // Prepare a file inside a subdir; update it via cd && apply_patch heredoc form.
    harness.write_file("sub/in_sub.txt", "before\n").await?;

    let script = "cd sub && apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: in_sub.txt\n@@\n-before\n+after\n*** End Patch\nEOF\n";
    let call_id = "shell-heredoc-cd";
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_shell_command_call(call_id, script),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), bodies).await;

    harness.submit("apply via shell heredoc with cd").await?;

    let out = harness.function_call_stdout(call_id).await;
    assert!(
        out.contains("Success."),
        "expected successful apply_patch invocation via shell_command: {out}"
    );
    assert_eq!(harness.read_file_text("sub/in_sub.txt").await?, "after\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_cli_can_use_shell_command_output_as_patch_input() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "shell_command output producer runs in the test runner, not in the remote apply_patch workspace",
    );

    let harness =
        apply_patch_harness_with(|builder| builder.with_model("gpt-5.4").with_windows_cmd_shell())
            .await?;

    let source_contents = "line1\nnaïve café\nline3\n";
    harness.write_file("source.txt", source_contents).await?;

    let read_call_id = "read-source";
    let apply_call_id = "apply-from-read";

    fn stdout_from_shell_output(output: &str) -> String {
        let normalized = output.replace("\r\n", "\n").replace('\r', "\n");
        normalized
            .split_once("Output:\n")
            .map(|x| x.1)
            .unwrap_or("")
            .trim_end_matches('\n')
            .to_string()
    }

    fn function_call_output_text(body: &serde_json::Value, call_id: &str) -> String {
        body.get("input")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| {
                items.iter().find(|item| {
                    item.get("type").and_then(serde_json::Value::as_str)
                        == Some("function_call_output")
                        && item.get("call_id").and_then(serde_json::Value::as_str) == Some(call_id)
                })
            })
            .and_then(|item| item.get("output").and_then(serde_json::Value::as_str))
            .expect("function_call_output output string")
            .to_string()
    }

    struct DynamicApplyFromRead {
        num_calls: AtomicI32,
        read_call_id: String,
        apply_call_id: String,
    }

    impl Respond for DynamicApplyFromRead {
        fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
            let call_num = self.num_calls.fetch_add(1, Ordering::SeqCst);
            match call_num {
                0 => {
                    let command = if cfg!(windows) {
                        // Encode the nested PowerShell script so `cmd.exe /c` does not leave the
                        // read command wrapped in quotes, and suppress progress records so the
                        // shell tool only returns the file contents back to apply_patch.
                        let script = "$ProgressPreference = 'SilentlyContinue'; [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); [System.IO.File]::ReadAllText('source.txt', [System.Text.UTF8Encoding]::new($false))";
                        let encoded = BASE64_STANDARD.encode(
                            script
                                .encode_utf16()
                                .flat_map(u16::to_le_bytes)
                                .collect::<Vec<u8>>(),
                        );
                        format!(
                            "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {encoded}"
                        )
                    } else {
                        "cat source.txt".to_string()
                    };
                    let args = json!({
                        "command": command,
                        "login": false,
                    });
                    let body = sse(vec![
                        ev_response_created("resp-1"),
                        ev_shell_command_call_with_args(&self.read_call_id, &args),
                        ev_completed("resp-1"),
                    ]);
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "text/event-stream")
                        .set_body_string(body)
                }
                1 => {
                    let body_json: serde_json::Value =
                        request.body_json().expect("request body should be json");
                    let read_output = function_call_output_text(&body_json, &self.read_call_id);
                    let stdout = stdout_from_shell_output(&read_output);
                    let patch_lines = stdout
                        .lines()
                        .map(|line| format!("+{line}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let patch = format!(
                        "*** Begin Patch\n*** Add File: target.txt\n{patch_lines}\n*** End Patch"
                    );

                    let body = sse(vec![
                        ev_response_created("resp-2"),
                        ev_apply_patch_custom_tool_call(&self.apply_call_id, &patch),
                        ev_completed("resp-2"),
                    ]);
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "text/event-stream")
                        .set_body_string(body)
                }
                2 => {
                    let body = sse(vec![
                        ev_assistant_message("msg-1", "ok"),
                        ev_completed("resp-3"),
                    ]);
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "text/event-stream")
                        .set_body_string(body)
                }
                _ => panic!("no response for call {call_num}"),
            }
        }
    }

    let responder = DynamicApplyFromRead {
        num_calls: AtomicI32::new(0),
        read_call_id: read_call_id.to_string(),
        apply_call_id: apply_call_id.to_string(),
    };
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responder)
        .expect(3)
        .mount(harness.server())
        .await;

    harness
        .submit("read source.txt, then apply it to target.txt")
        .await?;

    let target_contents = harness.read_file_text("target.txt").await?;
    assert_eq!(target_contents, source_contents);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_streaming_emits_updated_changes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness_with(|builder| {
        builder.with_config(|config| {
            config
                .features
                .enable(Feature::ApplyPatchStreamingEvents)
                .expect("enable apply_patch streaming events");
        })
    })
    .await?;
    let test = harness.test();
    let codex = test.codex.clone();
    let call_id = "apply-patch-streaming";
    let patch = "*** Begin Patch\n*** Add File: streamed.txt\n+hello\n+world\n*** End Patch";
    mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                json!({
                    "type": "response.output_item.added",
                    "item": {
                        "type": "custom_tool_call",
                        "call_id": call_id,
                        "name": "apply_patch",
                        "input": "",
                    }
                }),
                json!({
                    "type": "response.custom_tool_call_input.delta",
                    "call_id": call_id,
                    "delta": "*** Begin Patch\n",
                }),
                json!({
                    "type": "response.custom_tool_call_input.delta",
                    "call_id": call_id,
                    "delta": "*** Add File: streamed.txt\n+hello",
                }),
                json!({
                    "type": "response.custom_tool_call_input.delta",
                    "call_id": call_id,
                    "delta": "\n+world\n*** End Patch",
                }),
                ev_apply_patch_custom_tool_call(call_id, patch),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    codex
        .submit(Op::UserTurn {
            environments: None,
            items: vec![UserInput::Text {
                text: "create streamed file".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: harness.cwd().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            permission_profile: None,
            model: test.session_configured.model.clone(),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut updates = Vec::new();
    wait_for_event(&codex, |event| match event {
        EventMsg::PatchApplyUpdated(update) => {
            updates.push(update.clone());
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    assert_eq!(
        updates
            .iter()
            .map(|update| update.call_id.as_str())
            .collect::<Vec<_>>(),
        vec![call_id, call_id]
    );
    assert_eq!(
        updates
            .first()
            .expect("first update")
            .changes
            .get(&std::path::PathBuf::from("streamed.txt")),
        Some(&codex_protocol::protocol::FileChange::Add {
            content: "hello\n".to_string(),
        })
    );
    assert_eq!(
        updates
            .last()
            .expect("last update")
            .changes
            .get(&std::path::PathBuf::from("streamed.txt")),
        Some(&codex_protocol::protocol::FileChange::Add {
            content: "hello\nworld\n".to_string(),
        })
    );
    assert_eq!(
        harness.read_file_text("streamed.txt").await?,
        "hello\nworld\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_shell_command_heredoc_with_cd_emits_turn_diff() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "TurnDiffTracker currently reads the test-runner filesystem, not the remote executor filesystem",
    );

    let harness = apply_patch_harness_with(|builder| builder.with_model("gpt-5.4")).await?;
    let test = harness.test();
    let codex = test.codex.clone();

    // Prepare a file inside a subdir; update it via cd && apply_patch heredoc form.
    harness.write_file("sub/in_sub.txt", "before\n").await?;

    let script = "cd sub && apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: in_sub.txt\n@@\n-before\n+after\n*** End Patch\nEOF\n";
    let call_id = "shell-heredoc-cd";
    let args = json!({ "command": script, "timeout_ms": 30_000 });
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "ok"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), bodies).await;

    let model = test.session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            environments: None,
            items: vec![UserInput::Text {
                text: "apply via shell heredoc with cd".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: harness.cwd().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            permission_profile: None,
            model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut saw_turn_diff = None;
    let mut saw_patch_begin = false;
    let mut patch_end_success = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::PatchApplyBegin(begin) => {
            saw_patch_begin = true;
            assert_eq!(begin.call_id, call_id);
            false
        }
        EventMsg::PatchApplyEnd(end) => {
            assert_eq!(end.call_id, call_id);
            patch_end_success = Some(end.success);
            false
        }
        EventMsg::TurnDiff(ev) => {
            saw_turn_diff = Some(ev.unified_diff.clone());
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    assert!(saw_patch_begin, "expected PatchApplyBegin event");
    let patch_end_success =
        patch_end_success.expect("expected PatchApplyEnd event to capture success flag");
    assert!(patch_end_success);

    let diff = saw_turn_diff.expect("expected TurnDiff event");
    assert!(diff.contains("diff --git"), "diff header missing: {diff:?}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_shell_command_failure_propagates_error_and_skips_diff() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "TurnDiffTracker currently reads the test-runner filesystem, not the remote executor filesystem",
    );

    let harness = apply_patch_harness_with(|builder| builder.with_model("gpt-5.4")).await?;
    let test = harness.test();
    let codex = test.codex.clone();

    harness.write_file("invalid.txt", "ok\n").await?;

    let script = "apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: invalid.txt\n@@\n-nope\n+changed\n*** End Patch\nEOF\n";
    let call_id = "shell-apply-failure";
    let args = json!({ "command": script, "timeout_ms": 5_000 });
    let bodies = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "fail"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), bodies).await;

    let model = test.session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            environments: None,
            items: vec![UserInput::Text {
                text: "apply patch via shell".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: harness.cwd().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            permission_profile: None,
            model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut saw_turn_diff = false;
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnDiff(_) => {
            saw_turn_diff = true;
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    assert!(
        !saw_turn_diff,
        "turn diff should not be emitted when shell apply_patch fails verification"
    );

    let out = harness.function_call_stdout(call_id).await;
    assert!(
        out.contains("Failed to find expected lines in"),
        "expected failure diagnostics: {out}"
    );
    assert!(
        out.contains("invalid.txt"),
        "expected file path in output: {out}"
    );
    assert_eq!(harness.read_file_text("invalid.txt").await?, "ok\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_function_accepts_lenient_heredoc_wrapped_patch(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let file_name = "lenient.txt";
    let patch_inner =
        format!("*** Begin Patch\n*** Add File: {file_name}\n+lenient\n*** End Patch\n");
    let call_id = "apply-lenient";
    mount_apply_patch(&harness, call_id, patch_inner.as_str(), "ok", model_output).await;

    harness.submit("apply lenient heredoc patch").await?;

    assert_eq!(harness.read_file_text(file_name).await?, "lenient\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_end_of_file_anchor(model_output: ApplyPatchModelOutput) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness.write_file("tail.txt", "alpha\nlast\n").await?;

    let patch = "*** Begin Patch\n*** Update File: tail.txt\n@@\n-last\n+end\n*** End of File\n*** End Patch";
    let call_id = "apply-eof";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("apply EOF-anchored patch").await?;
    assert_eq!(harness.read_file_text("tail.txt").await?, "alpha\nend\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_cli_missing_second_chunk_context_rejected(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness.write_file("two_chunks.txt", "a\nb\nc\nd\n").await?;

    // First chunk has @@, second chunk intentionally omits @@ to trigger parse error.
    let patch =
        "*** Begin Patch\n*** Update File: two_chunks.txt\n@@\n-b\n+B\n\n-d\n+D\n*** End Patch";
    let call_id = "apply-missing-ctx-2nd";
    mount_apply_patch(&harness, call_id, patch, "fail", model_output).await;

    harness.submit("apply missing context second chunk").await?;

    let out = harness.apply_patch_output(call_id, model_output).await;
    assert!(out.contains("apply_patch verification failed"));
    assert!(
        out.contains("Failed to find expected lines in"),
        "expected hunk context diagnostics: {out}"
    );
    // Original file unchanged on failure
    assert_eq!(
        harness.read_file_text("two_chunks.txt").await?,
        "a\nb\nc\nd\n"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_emits_turn_diff_event_with_unified_diff(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "TurnDiffTracker currently reads the test-runner filesystem, not the remote executor filesystem",
    );

    let harness = apply_patch_harness().await?;
    let test = harness.test();
    let codex = test.codex.clone();

    let call_id = "apply-diff-event";
    let file = "udiff.txt";
    let patch = format!("*** Begin Patch\n*** Add File: {file}\n+hello\n*** End Patch\n");
    mount_apply_patch(&harness, call_id, patch.as_str(), "ok", model_output).await;

    let model = test.session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            environments: None,
            items: vec![UserInput::Text {
                text: "emit diff".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: harness.cwd().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            permission_profile: None,
            model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut saw_turn_diff = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnDiff(ev) => {
            saw_turn_diff = Some(ev.unified_diff.clone());
            false
        }
        EventMsg::TurnComplete(_) => true,
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
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_turn_diff_for_rename_with_content_change(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "TurnDiffTracker currently reads the test-runner filesystem, not the remote executor filesystem",
    );

    let harness = apply_patch_harness().await?;
    let test = harness.test();
    let codex = test.codex.clone();

    // Seed original file
    harness.write_file("old.txt", "old\n").await?;

    // Patch: update + move
    let call_id = "apply-rename-change";
    let patch = "*** Begin Patch\n*** Update File: old.txt\n*** Move to: new.txt\n@@\n-old\n+new\n*** End Patch";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    let model = test.session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            environments: None,
            items: vec![UserInput::Text {
                text: "rename with change".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: harness.cwd().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            permission_profile: None,
            model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut last_diff: Option<String> = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnDiff(ev) => {
            last_diff = Some(ev.unified_diff.clone());
            false
        }
        EventMsg::TurnComplete(_) => true,
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
    skip_if_remote!(
        Ok(()),
        "TurnDiffTracker currently reads the test-runner filesystem, not the remote executor filesystem",
    );

    let harness = apply_patch_harness().await?;
    let test = harness.test();
    let codex = test.codex.clone();

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
    mount_sse_sequence(harness.server(), vec![s1, s2, s3]).await;

    let model = test.session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            environments: None,
            items: vec![UserInput::Text {
                text: "aggregate diffs".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: harness.cwd().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            permission_profile: None,
            model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut last_diff: Option<String> = None;
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnDiff(ev) => {
            last_diff = Some(ev.unified_diff.clone());
            false
        }
        EventMsg::TurnComplete(_) => true,
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
async fn apply_patch_aggregates_diff_preserves_success_after_failure() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "TurnDiffTracker currently reads the test-runner filesystem, not the remote executor filesystem",
    );

    let harness = apply_patch_harness().await?;
    let test = harness.test();
    let codex = test.codex.clone();

    let call_success = "agg-success";
    let call_failure = "agg-failure";
    let patch_success = "*** Begin Patch\n*** Add File: partial/success.txt\n+ok\n*** End Patch";
    let patch_failure =
        "*** Begin Patch\n*** Update File: partial/success.txt\n@@\n-missing\n+new\n*** End Patch";

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_success, patch_success),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_apply_patch_function_call(call_failure, patch_failure),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "failed"),
            ev_completed("resp-3"),
        ]),
    ];
    mount_sse_sequence(harness.server(), responses).await;

    let model = test.session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            environments: None,
            items: vec![UserInput::Text {
                text: "apply patch twice with failure".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: harness.cwd().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            permission_profile: None,
            model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let mut last_diff: Option<String> = None;
    wait_for_event_with_timeout(
        &codex,
        |event| match event {
            EventMsg::TurnDiff(ev) => {
                last_diff = Some(ev.unified_diff.clone());
                false
            }
            EventMsg::TurnComplete(_) => true,
            _ => false,
        },
        Duration::from_secs(30),
    )
    .await;

    let diff = last_diff.expect("expected TurnDiff after failed patch");
    assert!(
        diff.contains("partial/success.txt"),
        "diff should still include the successful addition: {diff}"
    );
    assert!(
        diff.contains("+ok"),
        "diff should include contents from successful patch: {diff}"
    );

    let failure_out = harness.function_call_stdout(call_failure).await;
    assert!(
        failure_out.contains("apply_patch verification failed"),
        "expected verification failure output: {failure_out}"
    );
    assert!(
        failure_out.contains("Failed to find expected lines in"),
        "expected missing context diagnostics: {failure_out}"
    );

    assert_eq!(harness.read_file_text("partial/success.txt").await?, "ok\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(ApplyPatchModelOutput::Freeform)]
#[test_case(ApplyPatchModelOutput::Function)]
#[test_case(ApplyPatchModelOutput::Shell)]
#[test_case(ApplyPatchModelOutput::ShellViaHeredoc)]
#[test_case(ApplyPatchModelOutput::ShellCommandViaHeredoc)]
async fn apply_patch_change_context_disambiguates_target(
    model_output: ApplyPatchModelOutput,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    harness
        .write_file("multi_ctx.txt", "fn a\nx=10\ny=2\nfn b\nx=10\ny=20\n")
        .await?;

    let patch =
        "*** Begin Patch\n*** Update File: multi_ctx.txt\n@@ fn b\n-x=10\n+x=11\n*** End Patch";
    let call_id = "apply-ctx";
    mount_apply_patch(&harness, call_id, patch, "ok", model_output).await;

    harness.submit("apply with change_context").await?;

    let contents = harness.read_file_text("multi_ctx.txt").await?;
    assert_eq!(contents, "fn a\nx=10\ny=2\nfn b\nx=11\ny=20\n");
    Ok(())
}
