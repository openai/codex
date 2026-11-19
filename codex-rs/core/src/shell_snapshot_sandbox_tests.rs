use super::*;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::sandbox::SandboxOverride;
use codex_sandboxing::SandboxType;
use core_test_support::PathExt;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use test_case::test_case;

#[derive(Clone, Copy)]
enum SnapshotFailure {
    Exit,
    WorkspaceRootDenial,
    Timeout,
    NetworkDenial,
    CallerDrop,
    CallerDropWithNetworkToken,
}

#[test_case(SnapshotFailure::Exit; "startup_exit")]
#[test_case(SnapshotFailure::WorkspaceRootDenial; "denied_read_in_second_workspace_root")]
#[test_case(SnapshotFailure::Timeout; "timeout")]
#[test_case(SnapshotFailure::NetworkDenial; "network_denial")]
#[test_case(SnapshotFailure::CallerDrop; "caller_drop")]
#[test_case(SnapshotFailure::CallerDropWithNetworkToken; "caller_drop_preserves_network_token")]
#[tokio::test]
async fn snapshot_failure_omits_credentials_and_stops_descendants(
    failure: SnapshotFailure,
) -> Result<()> {
    let cancellation = match failure {
        SnapshotFailure::NetworkDenial | SnapshotFailure::CallerDropWithNetworkToken => {
            Some(CancellationToken::new())
        }
        SnapshotFailure::Exit
        | SnapshotFailure::WorkspaceRootDenial
        | SnapshotFailure::Timeout
        | SnapshotFailure::CallerDrop => None,
    };
    let dir = tempdir()?;
    let cwd = dir.path().join("first").abs();
    std::fs::create_dir(&cwd)?;
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let mut workspace_roots = vec![cwd_uri.clone()];
    let workspace_denial = matches!(failure, SnapshotFailure::WorkspaceRootDenial);
    let (permissions, local_selection, sandbox_exe) = if workspace_denial {
        core_test_support::skip_if_sandbox!(Ok(()));
        let second = dir.path().join("second").abs();
        std::fs::create_dir(&second)?;
        for root in [&cwd, &second] {
            std::fs::write(root.join("allowed"), "visible")?;
            std::fs::write(root.join("private"), "private contents")?;
        }
        workspace_roots.push(PathUri::from_abs_path(&second));
        let mut policy = FileSystemSandboxPolicy::read_only();
        policy.entries.push(FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some("private".into())),
            },
            FileSystemAccessMode::Deny,
        ));
        // This fixture only exercises filesystem restrictions. Remote runners can
        // deny bubblewrap's nested network namespace before the shell starts.
        (
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Enabled),
            SandboxSelection::Platform {
                preference: SandboxablePreference::Auto,
                windows_sandbox: SandboxType::None,
            },
            core_test_support::codex_linux_sandbox_exe_or_skip!(Ok(())),
        )
    } else {
        (
            PermissionProfile::Disabled,
            SandboxSelection::Explicit(SandboxType::None),
            None,
        )
    };
    let manager = SandboxManager::new();
    let attempt = SandboxAttempt {
        sandbox_override: SandboxOverride::NoOverride,
        local_selection,
        sandbox_requested: workspace_denial,
        permissions: &permissions,
        exec_server_permissions: &permissions,
        enforce_managed_network: false,
        manager: &manager,
        sandbox_cwd: &cwd_uri,
        workspace_roots: &workspace_roots,
        sandbox_exe: sandbox_exe.as_ref(),
        use_legacy_landlock: false,
        windows_sandbox_type: SandboxType::None,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        network_denial_cancellation_token: cancellation.clone(),
        network_proxy: None,
    };
    let sandbox = ShellSnapshotSandbox::new(
        &attempt, /*network*/ None, "local", /*additional_permissions*/ None,
    );
    let snapshot_timeout = match failure {
        SnapshotFailure::Exit
        | SnapshotFailure::WorkspaceRootDenial
        | SnapshotFailure::NetworkDenial
        | SnapshotFailure::CallerDrop
        | SnapshotFailure::CallerDropWithNetworkToken => Duration::from_secs(30),
        SnapshotFailure::Timeout => Duration::from_millis(250),
    };
    let script = match failure {
        SnapshotFailure::Exit => "exit 23",
        SnapshotFailure::WorkspaceRootDenial => {
            r#"for root in . ../second; do
                [ "$(/bin/cat "$root/allowed")" = visible ] || exit 41
            done
            if /bin/cat private >/dev/null 2>&1; then exit 42; fi
            if /bin/cat ../second/private >/dev/null 2>&1; then exit 43; fi
            exit 23"#
        }
        SnapshotFailure::Timeout
        | SnapshotFailure::NetworkDenial
        | SnapshotFailure::CallerDrop
        | SnapshotFailure::CallerDropWithNetworkToken => {
            "(printf ready > started; sleep 1; printf survived > late_write) & wait"
        }
    };
    let capture = sandbox.run(
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!("set -x; export GH_TOKEN=ghp_abcdefghijklmnopqrstuvwxyz0123456789; {script}"),
        ],
        &cwd,
        std::env::vars().collect(),
        snapshot_timeout,
        "sh",
        /*snapshot_read_path*/ None,
    );
    let cancel_after_startup = async {
        if !matches!(
            failure,
            SnapshotFailure::Exit | SnapshotFailure::WorkspaceRootDenial
        ) {
            tokio::time::timeout(Duration::from_secs(5), async {
                while !cwd.join("started").exists() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .context("startup should begin before cancellation")?;
            if matches!(failure, SnapshotFailure::NetworkDenial) {
                cancellation
                    .as_ref()
                    .context("network-denial test must have a cancellation token")?
                    .cancel();
            }
        }
        Ok::<(), anyhow::Error>(())
    };
    let result = tokio::time::timeout(Duration::from_secs(6), async {
        if matches!(
            failure,
            SnapshotFailure::CallerDrop | SnapshotFailure::CallerDropWithNetworkToken
        ) {
            let mut capture = Box::pin(capture);
            tokio::select! {
                result = &mut capture => bail!("capture completed before caller drop: {result:?}"),
                result = cancel_after_startup => result?,
            }
            drop(capture);
            assert!(
                cancellation
                    .as_ref()
                    .is_none_or(|token| !token.is_cancelled())
            );
            Ok(None)
        } else {
            let (result, cancellation_result) = tokio::join!(capture, cancel_after_startup);
            cancellation_result?;
            Ok::<_, anyhow::Error>(Some(result))
        }
    })
    .await
    .context("failed snapshot capture should return promptly")??;
    if let Some(result) = result {
        let error = result.err().context("failed snapshot must not be used")?;
        let expected = match failure {
            SnapshotFailure::Exit | SnapshotFailure::WorkspaceRootDenial => {
                "Snapshot command exited with status 23"
            }
            SnapshotFailure::Timeout => "Snapshot command timed out",
            SnapshotFailure::NetworkDenial => "Snapshot command exited with status 1",
            SnapshotFailure::CallerDrop | SnapshotFailure::CallerDropWithNetworkToken => {
                unreachable!("dropped captures have no result")
            }
        };
        assert_eq!(format!("{error:?}"), expected);
    }
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    assert!(
        !cwd.join("late_write").exists(),
        "startup descendant survived snapshot expiration"
    );
    Ok(())
}
