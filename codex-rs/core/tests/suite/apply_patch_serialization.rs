#![cfg(not(target_os = "windows"))]

use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use core_test_support::assert_regex_match;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_target_windows;
use pretty_assertions::assert_eq;

use crate::suite::apply_patch_cli::apply_patch_harness;
use crate::suite::apply_patch_cli::mount_apply_patch;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_call_creates_file() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let call_id = "apply-patch-add-file";
    let file_name = "custom_tool_apply_patch.txt";
    let patch = format!(
        "*** Begin Patch\n*** Add File: {file_name}\n+custom tool content\n*** End Patch\n"
    );
    mount_apply_patch(&harness, call_id, &patch, "apply_patch done").await;

    harness
        .test()
        .submit_turn_with_permission_profile(
            "apply the patch via custom tool to create a file",
            PermissionProfile::Disabled,
        )
        .await?;

    let output = harness.apply_patch_output(call_id).await;

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
A {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, output.as_str());

    let created_contents = harness.read_file_text(file_name).await?;
    assert_eq!(
        created_contents, "custom tool content\n",
        "expected file contents for {file_name}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_call_updates_existing_file() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let call_id = "apply-patch-update-file";
    let file_name = "custom_tool_apply_patch_existing.txt";
    harness.write_file(file_name, "before\n").await?;
    let patch = format!(
        "*** Begin Patch\n*** Update File: {file_name}\n@@\n-before\n+after\n*** End Patch\n"
    );
    mount_apply_patch(&harness, call_id, &patch, "apply_patch update done").await;

    harness
        .test()
        .submit_turn_with_permission_profile(
            "apply the patch via custom tool to update a file",
            PermissionProfile::Disabled,
        )
        .await?;

    let output = harness.apply_patch_output(call_id).await;

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
M {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, output.as_str());

    let updated_contents = harness.read_file_text(file_name).await?;
    assert_eq!(updated_contents, "after\n", "expected updated file content");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_call_reports_failure_output() -> Result<()> {
    // TODO(anp): Remove after apply-patch assertions use target-native paths.
    skip_if_target_windows!(Ok(()), "asserts POSIX apply_patch failure text");
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let call_id = "apply-patch-failure";
    let missing_file = "missing_custom_tool_apply_patch.txt";
    let patch = format!(
        "*** Begin Patch\n*** Update File: {missing_file}\n@@\n-before\n+after\n*** End Patch\n"
    );
    mount_apply_patch(&harness, call_id, &patch, "apply_patch failure done").await;

    harness
        .test()
        .submit_turn_with_permission_profile(
            "attempt a failing apply_patch via custom tool",
            PermissionProfile::Disabled,
        )
        .await?;

    let output = harness.apply_patch_output(call_id).await;

    let expected_output = format!(
        "apply_patch verification failed: Failed to read file to update {}/{missing_file}: No such file or directory (os error 2)",
        harness.cwd().to_string_lossy()
    );
    assert_eq!(output, expected_output.as_str());

    Ok(())
}
