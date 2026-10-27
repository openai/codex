//! Coverage for post-exit output delivery over a backpressured connection.
#![cfg(test)]

use super::*;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use codex_analytics::AnalyticsEventsClient;
use pretty_assertions::assert_eq;

fn start_collector(
    process_exited: CancellationToken,
) -> (
    mpsc::Sender<Vec<u8>>,
    mpsc::Receiver<OutgoingEnvelope>,
    tokio::task::JoinHandle<ProcessOutputCapture>,
) {
    let (output_tx, output_rx) = mpsc::channel(2);
    let (tx, notifications) = mpsc::channel(1);
    let collector = collect_spawn_process_output(SpawnProcessOutputParams {
        connection_id: ConnectionId(1),
        process_handle: "test".to_string(),
        output_rx,
        reader: None,
        process_exited,
        outgoing: Arc::new(OutgoingMessageSender::new(
            tx,
            AnalyticsEventsClient::disabled(),
        )),
        stream: ProcessOutputStream::Stdout,
        stream_output: true,
        output_bytes_cap: None,
    });
    (output_tx, notifications, collector)
}

async fn read_delta(
    notifications: &mut mpsc::Receiver<OutgoingEnvelope>,
) -> (Vec<u8>, oneshot::Sender<()>) {
    let Some(OutgoingEnvelope::ToConnection {
        message: OutgoingMessage::AppServerNotification(envelope),
        write_complete_tx: Some(written),
        ..
    }) = notifications.recv().await
    else {
        panic!("expected an output notification");
    };
    let ServerNotification::ProcessOutputDelta(delta) = envelope.notification else {
        panic!("expected an output delta");
    };
    (STANDARD.decode(delta.delta_base64).unwrap(), written)
}

/// A client can take longer than the drain grace to accept each of several final chunks.
#[tokio::test(start_paused = true)]
async fn slow_writes_deliver_the_entire_tail_after_exit() {
    let exited = CancellationToken::new();
    let (output, mut notifications, collector) = start_collector(exited.clone());
    output.send(vec![1]).await.unwrap();
    let (first, written) = read_delta(&mut notifications).await;
    exited.cancel();
    output.send(vec![2]).await.unwrap();
    let slow_write = Duration::from_millis(IO_DRAIN_TIMEOUT_MS * 2);
    tokio::time::advance(slow_write).await;
    written.send(()).unwrap();

    let (second, written) = read_delta(&mut notifications).await;
    tokio::time::advance(slow_write).await;
    // A descendant can still produce bytes while the previous write is blocked.
    output.send(vec![3]).await.unwrap();
    written.send(()).unwrap();

    let (third, written) = read_delta(&mut notifications).await;
    drop(output);
    written.send(()).unwrap();
    assert_eq!([first, second, third], [vec![1], vec![2], vec![3]]);
    let capture = collector.await.unwrap();
    assert!(!capture.output_incomplete);
}

/// A descendant that holds a pipe open gets one shared grace period across reads.
#[tokio::test(start_paused = true)]
async fn inherited_pipe_does_not_restart_the_grace_period_for_each_chunk() {
    let exited = CancellationToken::new();
    exited.cancel();
    let (output, mut notifications, collector) = start_collector(exited);
    tokio::task::yield_now().await;
    let grace = Duration::from_millis(IO_DRAIN_TIMEOUT_MS);
    tokio::time::advance(grace / 2).await;
    output.send(vec![1]).await.unwrap();
    let (_, written) = read_delta(&mut notifications).await;
    written.send(()).unwrap();

    let capture = tokio::time::timeout(grace / 2 + Duration::from_millis(1), collector)
        .await
        .expect("the collector must finish within the original drain budget")
        .unwrap();
    assert!(capture.output_incomplete);
}

/// Even a capped stream closes its OS reader on a genuine timeout while its sibling stays open.
#[cfg(unix)]
#[tokio::test]
async fn timed_out_stderr_closes_while_stdout_is_still_open() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = codex_utils_pty::spawn_pipe_process_no_stdin(
        "/bin/sh",
        &[
            "-c".into(),
            concat!(
                "printf cap >&2; (while [ ! -f release ]; do sleep 0.05; done; ",
                "if dd if=/dev/zero bs=65536 count=32 >&2 2>/dev/null; ",
                "then printf open; else printf closed; fi) &",
            )
            .into(),
        ],
        dir.path(),
        &HashMap::new(),
        &None,
        &[],
    )
    .await
    .unwrap();
    let (tx, _notifications) = mpsc::channel(1);
    let process_exited = CancellationToken::new();
    let collector = collect_spawn_process_output(SpawnProcessOutputParams {
        connection_id: ConnectionId(1),
        process_handle: "probe".into(),
        output_rx: child.stderr_rx,
        reader: child.session.stderr_reader(),
        process_exited: process_exited.clone(),
        outgoing: Arc::new(OutgoingMessageSender::new(
            tx,
            AnalyticsEventsClient::disabled(),
        )),
        stream: ProcessOutputStream::Stderr,
        stream_output: false,
        output_bytes_cap: Some(3),
    });
    assert_eq!(child.exit_rx.await.unwrap(), 0);
    process_exited.cancel();
    let capture = collector.await.unwrap();
    assert_eq!(capture.text, "cap");
    assert!(capture.cap_reached);
    assert!(!capture.output_incomplete);
    std::fs::write(dir.path().join("release"), b"").unwrap();
    let mut stdout = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(bytes) = child.stdout_rx.recv().await {
            stdout.extend(bytes);
        }
    })
    .await
    .expect("the descendant must observe the closed stderr pipe");
    assert_eq!(stdout, b"closed");
}

/// The client must receive the root's exit while a surviving Windows console child can still run.
#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn windows_terminal_reports_exit_before_closing_the_console() {
    let dir = tempfile::tempdir().unwrap();
    // Bound the lifetime of the descendant even if the server fails to deliver the exit.
    std::fs::write(
        dir.path().join("child.cmd"),
        concat!(
            "@echo off\r\n",
            "for /l %%i in (1,1,30) do (\r\n",
            "  if exist received goto ack\r\n",
            "  ping.exe -n 2 127.0.0.1 >nul\r\n",
            ")\r\nexit /b 1\r\n:ack\r\ntype nul >acknowledged\r\n",
        ),
    )
    .unwrap();
    let spawned = codex_utils_pty::spawn_pty_process(
        "cmd.exe",
        &[
            "/d".into(),
            "/c".into(),
            "echo complete & start /b cmd.exe /d /c child.cmd".into(),
        ],
        dir.path(),
        &std::env::vars().collect(),
        /*arg0*/ &None,
        TerminalSize::default(),
        codex_utils_pty::ChildFds::Inherited(&[]),
    )
    .await
    .unwrap();
    let (_control_tx, control_rx) = mpsc::channel(1);
    let (tx, mut notifications) = mpsc::channel(1);
    let process = tokio::spawn(run_process(RunProcessParams {
        outgoing: Arc::new(OutgoingMessageSender::new(
            tx,
            AnalyticsEventsClient::disabled(),
        )),
        request_id: ConnectionRequestId {
            connection_id: ConnectionId(1),
            request_id: codex_app_server_protocol::RequestId::Integer(1),
        },
        process_handle: "console".into(),
        spawned,
        control_rx,
        stream_stdin: true,
        stream_stdout_stderr: false,
        expiration: ExecExpiration::DefaultTimeout,
        output_bytes_cap: None,
    }));
    let Some(OutgoingEnvelope::ToConnection {
        message: OutgoingMessage::AppServerNotification(envelope),
        write_complete_tx: Some(written),
        ..
    }) = tokio::time::timeout(Duration::from_secs(15), notifications.recv())
        .await
        .expect("exit should arrive while the descendant is still attached")
    else {
        panic!("expected an exit notification")
    };
    let ServerNotification::ProcessExited(exit) = envelope.notification else {
        panic!("expected process/exited");
    };
    assert_eq!(exit.exit_code, 0);
    assert!(exit.stdout.contains("complete"));
    assert_eq!(exit.output_drain_timed_out, Some(true));
    std::fs::write(dir.path().join("received"), b"").unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !dir.path().join("acknowledged").exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the attached child should still be alive when the client receives process/exited");
    written.send(()).unwrap();
    process.await.unwrap();
}
