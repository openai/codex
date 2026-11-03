//! Exercise the real helper protocol and bounded process I/O.

mod common;

use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_realtime_webrtc::Message;
use codex_realtime_webrtc::decode_frame;
use codex_realtime_webrtc::encode_frame;
use common::voice_host_command;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Child;
use tokio::time::timeout;

const DEADLINE: Duration = Duration::from_secs(/*secs*/ 10);

fn spawn() -> Result<Child> {
    Ok(voice_host_command()?
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?)
}

async fn build_commit() -> Result<String> {
    let output = timeout(
        DEADLINE,
        voice_host_command()?
            .arg("--build-commit")
            .kill_on_drop(true)
            .output(),
    )
    .await??;
    assert!(output.status.success());
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

async fn handshake(child: &mut Child) -> Result<()> {
    child
        .stdin
        .as_mut()
        .context("stdin")?
        .write_all(&encode_frame(&Message::Hello {
            protocol: 1,
            build_commit: build_commit().await?,
        })?)
        .await?;
    let expected = encode_frame(&Message::Ready {})?;
    let mut reply = vec![0; expected.len()];
    timeout(
        DEADLINE,
        child
            .stdout
            .as_mut()
            .context("stdout")?
            .read_exact(&mut reply),
    )
    .await??;
    assert_eq!(reply, expected);
    Ok(())
}

#[tokio::test]
async fn closes_after_acknowledgement_and_on_parent_pipe_loss() -> Result<()> {
    for explicit_close in [true, false] {
        let mut child = spawn()?;
        handshake(&mut child).await?;
        if explicit_close {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(&encode_frame(&Message::Close {})?)
                .await?;
        }
        drop(child.stdin.take());
        let output = timeout(DEADLINE, child.wait_with_output()).await??;
        assert!(output.status.success());
        assert_eq!(
            output.stdout,
            if explicit_close {
                encode_frame(&Message::Closed {})?
            } else {
                vec![]
            }
        );
        assert!(output.stderr.is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn parent_pipe_loss_cancels_transport_startup() -> Result<()> {
    let mut child = spawn()?;
    handshake(&mut child).await?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&encode_frame(&Message::StartTransport {})?)
        .await?;
    let output = timeout(Duration::from_secs(/*secs*/ 5), child.wait_with_output()).await??;
    assert!(output.stderr.is_empty());
    Ok(())
}

#[tokio::test]
async fn rejects_invalid_input_without_echoing_it() -> Result<()> {
    assert!(decode_frame(b"\0\0\0\x16{\"type\":\"close\",\"x\":0}").is_err());
    let mut invalid_json = 22_u32.to_be_bytes().to_vec();
    invalid_json.extend_from_slice(b"sensitive-invalid-json");
    for frame in [
        u32::MAX.to_be_bytes().to_vec(),
        vec![0, 0],
        invalid_json,
        encode_frame(&Message::Hello {
            protocol: 1,
            build_commit: "wrong-build".into(),
        })?,
        encode_frame(&Message::Hello {
            protocol: 99,
            build_commit: build_commit().await?,
        })?,
        encode_frame(&Message::Close {})?,
    ] {
        let mut child = spawn()?;
        child.stdin.take().unwrap().write_all(&frame).await?;
        let output = timeout(DEADLINE, child.wait_with_output()).await??;
        assert!(!output.status.success());
        assert_eq!((output.stdout, output.stderr), (vec![], vec![]));
    }
    Ok(())
}
