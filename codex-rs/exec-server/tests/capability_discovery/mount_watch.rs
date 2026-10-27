//! Exercises missing-path watches across mount changes on an idle executor WebSocket.

use super::common::exec_server::ExecServerHarness;
use super::common::exec_server::exec_server;
use super::discover_root;
use super::initialize;
use super::write_file;
use anyhow::Context as _;
use codex_exec_server_protocol::FS_MISSING_CHANGED_METHOD;
use codex_exec_server_protocol::FS_WATCH_MISSING_METHOD;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCNotification;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::process::Command;

const PARENT_NAMESPACE: &str = "CODEX_TEST_MISSING_WATCH_PARENT_MOUNT_NS";

#[test]
fn bind_mount_and_unmount_reveal_skills() -> anyhow::Result<()> {
    // The parent removes the fixtures after the child's private mounts disappear.
    let tmp = tempfile::tempdir()?;
    let output = Command::new("unshare")
        .args([
            "--user",
            "--map-root-user",
            "--mount",
            "--propagation",
            "private",
            "--",
        ])
        .arg(std::env::current_exe()?)
        .args([
            "--exact",
            "mount_watch::in_private_namespace",
            "--ignored",
            "--nocapture",
        ])
        .env(PARENT_NAMESPACE, std::fs::read_link("/proc/self/ns/mnt")?)
        .env("LC_ALL", "C")
        .env("TMPDIR", tmp.path())
        .output();
    if let Err(error) = &output
        && error.kind() == std::io::ErrorKind::NotFound
    {
        eprintln!("skipping mount watch test: unshare unavailable");
        return Ok(());
    }
    let output = output?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.code() == Some(1)
        && stderr.starts_with("unshare:")
        && [
            "Operation not permitted",
            "Permission denied",
            "Read-only file system",
            "Function not implemented",
        ]
        .iter()
        .any(|reason| stderr.contains(reason))
    {
        eprintln!("skipping mount watch test: {stderr}");
        return Ok(());
    }
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed;"),
        "{output:?}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "run by bind_mount_and_unmount_reveal_skills in a private Linux mount namespace"]
async fn in_private_namespace() -> anyhow::Result<()> {
    let parent = std::env::var_os(PARENT_NAMESPACE).context("use the namespace wrapper")?;
    assert_ne!(std::fs::read_link("/proc/self/ns/mnt")?.as_os_str(), parent);
    let fixture = tempfile::tempdir()?;
    let empty = fixture.path().join("empty/.agents");
    let populated = fixture.path().join("populated/.agents");
    std::fs::create_dir_all(&empty)?;
    write_file(
        &populated.join("skills/mounted/SKILL.md"),
        "---\nname: mounted\ndescription: Mount test\n---\n",
    )?;
    let mut server = exec_server().await?;
    initialize(&mut server).await?;

    assert_reveals_skills(
        &mut server,
        &empty,
        /*watch_id*/ 1,
        Command::new("mount")
            .arg("--bind")
            .arg(&populated)
            .arg(&empty),
    )
    .await?;
    run(Command::new("umount").arg(&empty))?;

    run(Command::new("mount")
        .arg("--bind")
        .arg(&empty)
        .arg(&populated))?;
    assert_reveals_skills(
        &mut server,
        &populated,
        /*watch_id*/ 2,
        Command::new("umount").arg(&populated),
    )
    .await?;
    server.shutdown().await?;
    Ok(())
}

async fn assert_reveals_skills(
    server: &mut ExecServerHarness,
    agents: &Path,
    watch_id: u64,
    change: &mut Command,
) -> anyhow::Result<()> {
    let root = PathUri::from_host_native_path(agents)?;
    let before = discover_root(server, "repository", root.clone()).await?;
    assert_eq!((before.error, before.skills), (None, vec![]));

    let id = server
        .send_request(
            FS_WATCH_MISSING_METHOD,
            serde_json::json!({ "watchId": watch_id, "missing": [root.join("skills")?] }),
        )
        .await?;
    assert_eq!(
        server.next_event().await?,
        JSONRPCMessage::Response(JSONRPCResponse {
            id,
            result: serde_json::json!({ "watching": true })
        })
    );
    run(change)?;
    assert_eq!(
        server
            .next_event()
            .await
            .context("expected a mount change on the idle socket")?,
        JSONRPCMessage::Notification(JSONRPCNotification {
            method: FS_MISSING_CHANGED_METHOD.to_string(),
            params: Some(serde_json::json!({ "watchId": watch_id })),
        })
    );

    let after = discover_root(server, "repository", root.clone()).await?;
    let skills: Vec<_> = after
        .skills
        .into_iter()
        .map(|skill| skill.instructions.path)
        .collect();
    assert_eq!(
        (after.error, skills),
        (None, vec![root.join("skills/mounted/SKILL.md")?])
    );
    Ok(())
}

fn run(command: &mut Command) -> anyhow::Result<()> {
    let output = command.output()?;
    anyhow::ensure!(
        output.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
