use super::*;
use core_test_support::PathExt;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use test_case::test_case;

#[derive(Clone, Copy)]
enum SnapshotFailure {
    Exit,
    Timeout,
    NetworkDenial,
    CallerDrop,
    CallerDropWithNetworkToken,
}

#[test_case(SnapshotFailure::Exit; "startup_exit")]
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
        SnapshotFailure::Exit | SnapshotFailure::Timeout | SnapshotFailure::CallerDrop => None,
    };
    let dir = tempdir()?;
    let cwd = dir.path().abs();
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let permissions = PermissionProfile::Disabled;
    let manager = SandboxManager::new();
    let attempt = SandboxAttempt {
        sandbox: SandboxType::None,
        sandbox_requested: false,
        permissions: &permissions,
        exec_server_permissions: &permissions,
        enforce_managed_network: false,
        manager: &manager,
        sandbox_cwd: &cwd_uri,
        workspace_roots: std::slice::from_ref(&cwd_uri),
        sandbox_exe: None,
        use_legacy_landlock: false,
        windows_sandbox_type: SandboxType::None,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        network_denial_cancellation_token: cancellation.clone(),
        network_proxy: None,
    };
    let sandbox = ShellSnapshotSandbox::new(
        &attempt, /*network*/ None, "local", /*additional_permissions*/ None,
    );
    let snapshot_timeout = match failure {
        SnapshotFailure::Exit
        | SnapshotFailure::NetworkDenial
        | SnapshotFailure::CallerDrop
        | SnapshotFailure::CallerDropWithNetworkToken => Duration::from_secs(30),
        SnapshotFailure::Timeout => Duration::from_millis(250),
    };
    let script = match failure {
        SnapshotFailure::Exit => "exit 23",
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
        if !matches!(failure, SnapshotFailure::Exit) {
            tokio::time::timeout(Duration::from_secs(5), async {
                while !dir.path().join("started").exists() {
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
            SnapshotFailure::Exit => "Snapshot command exited with status 23",
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
        !dir.path().join("late_write").exists(),
        "startup descendant survived snapshot expiration"
    );
    Ok(())
}
