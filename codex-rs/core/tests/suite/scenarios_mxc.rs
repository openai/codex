//! Exercises resolved local MXC selection in a real thread and snapshots its permission context.

use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::sandbox::SandboxType;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses::ev_completed;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

#[test_case::test_case(true; "preferred")]
#[test_case::test_case(false; "legacy")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preferred_mxc_reaches_local_thread_and_permission_context(prefer_mxc: bool) -> Result<()> {
    let server = start_mock_server().await;
    let mock = mount_sse_once(&server, sse(vec![ev_completed("done")])).await;
    let test = test_codex()
        .with_config(move |config| {
            // Start from the resolved preference; native probing is covered by config tests.
            config.prefer_mxc = prefer_mxc;
            config.permissions.windows_sandbox_type = SandboxType::None;
            config
                .permissions
                .set_permission_profile(if prefer_mxc {
                    PermissionProfile::workspace_write_with(
                        &[],
                        NetworkSandboxPolicy::Restricted,
                        /*exclude_tmpdir_env_var*/ true,
                        /*exclude_slash_tmp*/ true,
                    )
                } else {
                    PermissionProfile::read_only()
                })
                .expect("set test permissions");
        })
        // This explicitly exercises host-local selection, including on remote-executor CI.
        .build(&server)
        .await?;
    test.submit_text_turn("Inspect the sandbox.").await?;
    assert_eq!(
        codex_core::test_support::environment_windows_sandbox_types(&test.codex).await,
        vec![(
            "local".to_string(),
            if prefer_mxc {
                SandboxType::WindowsMxc
            } else {
                SandboxType::None
            }
        )]
    );
    if prefer_mxc {
        insta::assert_snapshot!(
            "preferred_mxc_permissions",
            context_snapshot::format_request_history_snapshot(
                "Resolved local MXC selection preserves workspace-write permissions in a real agent turn.",
                &mock.requests(),
                &ContextSnapshotOptions::default(),
            )
        );
    }
    Ok(())
}
