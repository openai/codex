use std::io;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use pretty_assertions::assert_eq;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::timeout;

use super::signal_process_group_with_member_fallback;
use super::signal_process_id;
use super::terminate_process_group_with_member_fallback;

#[tokio::test]
async fn denied_group_signal_terminates_owned_descendants_and_preserves_escalation() -> Result<()> {
    for leader_exited in [false, true] {
        let mut wrapper = Command::new("/bin/sh")
            .args([
                "-c",
                "trap '' TERM; /bin/sleep 30 & resistant=$!; trap - TERM; /bin/sleep 30 & sibling=$!; printf '%s %s\\n' \"$resistant\" \"$sibling\"; wait",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .process_group(0)
            .spawn()?;
        let process_group_id =
            wrapper.id().context("wrapper process has no process ID")? as libc::pid_t;
        let stdout = wrapper.stdout.take().context("wrapper has no stdout")?;
        let line = timeout(
            Duration::from_secs(5),
            BufReader::new(stdout).lines().next_line(),
        )
        .await??
        .context("missing descendant IDs")?;
        let (resistant_pid, sibling_pid) =
            line.split_once(' ').context("invalid descendant IDs")?;
        let resistant_pid = resistant_pid.parse::<libc::pid_t>()?;
        let sibling_pid = sibling_pid.parse::<libc::pid_t>()?;

        if leader_exited {
            wrapper.kill().await?;
        }

        let mut denied_leader = false;
        for signal in [libc::SIGTERM, libc::SIGKILL] {
            assert!(signal_process_group_with_member_fallback(
                process_group_id as u32,
                signal,
                |_, _| Err(io::Error::from_raw_os_error(libc::EPERM)),
                |process_id, signal| {
                    if process_id == process_group_id {
                        denied_leader = true;
                        Err(io::Error::from_raw_os_error(libc::EPERM))
                    } else {
                        signal_process_id(process_id, signal)
                    }
                },
            )?);
            if signal == libc::SIGTERM {
                assert_eq!(denied_leader, !leader_exited);
                assert!(signal_process_id(resistant_pid, /*signal*/ 0)?);
            }
        }

        for process_id in [resistant_pid, sibling_pid] {
            timeout(Duration::from_secs(5), async move {
                while signal_process_id(process_id, /*signal*/ 0)? {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Ok::<(), io::Error>(())
            })
            .await??;
        }

        if !leader_exited {
            timeout(Duration::from_secs(5), wrapper.wait()).await??;
        }
    }

    Ok(())
}

#[test]
fn denied_group_signal_rejects_unsafe_process_group_ids() {
    for process_group_id in [0, u32::MAX] {
        let error = terminate_process_group_with_member_fallback(process_group_id)
            .expect_err("unsafe process group ID should be rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
