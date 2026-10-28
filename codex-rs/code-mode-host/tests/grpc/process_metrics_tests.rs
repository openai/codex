//! Process observations are independent of session state and HTTP/2 keepalive.

use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_code_mode::CodeModeProcessMetrics;
use codex_code_mode::CodeModeProcessMetricsSample;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Endpoint;

use super::*;

#[tokio::test]
async fn metrics_failures_and_deadlines_preserve_sessions_and_keepalive() -> Result<()> {
    let snapshot = CodeModeProcessMetrics {
        process_id: 42,
        process_start_time_seconds: 1_700_000_000.0,
        cpu_seconds_total: 1.5,
        resident_memory_bytes: 4096,
    };
    let expected = snapshot.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let collector_calls = calls.clone();
    let (release, blocked) = std::sync::mpsc::channel::<()>();
    let blocked = Mutex::new(blocked);
    let host = codex_code_mode_host::GrpcCodeModeHost::with_process_metrics(Arc::new(move || {
        match collector_calls.fetch_add(1, Ordering::SeqCst) {
            0 => Ok(snapshot.clone()),
            1 => Err("metrics unavailable".to_string()),
            2 => panic!("collector panic"),
            _ => {
                blocked
                    .lock()
                    .unwrap()
                    .recv()
                    .map_err(|error| error.to_string())?;
                Ok(snapshot.clone())
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let connections = Arc::new(AtomicUsize::new(0));
    let accepted = connections.clone();
    let incoming = TcpListenerStream::new(listener).inspect(move |connection| {
        if connection.is_ok() {
            accepted.fetch_add(1, Ordering::SeqCst);
        }
    });
    let server = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(grpc::code_mode_host_server::CodeModeHostServer::new(host))
            .serve_with_incoming(incoming)
            .await
    }));
    // Multiple keepalive intervals elapse while the collector is blocked, even
    // on this single-threaded runtime. A stalled HTTP/2 driver would lose state.
    let channel = Endpoint::from_shared(format!("http://{address}"))?
        .http2_keep_alive_interval(Duration::from_millis(/*millis*/ 50))
        .keep_alive_timeout(Duration::from_millis(/*millis*/ 100))
        .keep_alive_while_idle(/*enabled*/ true)
        .connect()
        .await?;
    let provider = GrpcCodeModeSessionProvider::with_channel(channel);
    let session = provider
        .create_session()
        .await
        .map_err(anyhow::Error::msg)?;
    execute(
        &session,
        request("store('survives', 42);"),
        Arc::new(NoopCodeModeSessionDelegate),
    )
    .await?;
    assert_eq!(
        provider
            .sample_process_metrics()
            .await
            .map_err(anyhow::Error::msg)?,
        CodeModeProcessMetricsSample::Snapshot(expected)
    );
    for cell in 2..=4 {
        assert!(provider.sample_process_metrics().await.is_err());
        let response = execute(
            &session,
            request("text(load('survives'));"),
            Arc::new(NoopCodeModeSessionDelegate),
        )
        .await?;
        assert_eq!(
            response,
            text_response(&cell.to_string(), "42", response.code_mode_host_duration())
        );
    }
    // A canceled blocked RPC retains its collector quota: no unbounded worker
    // threads are added, and neither the sample nor overload opens another channel.
    assert!(provider.sample_process_metrics().await.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert_eq!(connections.load(Ordering::SeqCst), 1);
    release.send(())?;
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    drop(server);
    Ok(())
}

/// A running server's collection precondition failure is not local host absence.
#[tokio::test]
async fn remote_metrics_precondition_is_a_failure() -> Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(
                grpc::code_mode_host_server::CodeModeHostServer::with_interceptor(
                    codex_code_mode_host::GrpcCodeModeHost::new(),
                    |_request| {
                        Err(tonic::Status::failed_precondition(
                            "collector is misconfigured",
                        ))
                    },
                ),
            )
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
    }));
    let channel = Endpoint::from_shared(format!("http://{address}"))?
        .connect()
        .await?;
    let provider = GrpcCodeModeSessionProvider::with_channel(channel);
    let error = provider.sample_process_metrics().await.unwrap_err();
    assert!(error.contains("collector is misconfigured"), "{error}");
    drop(server);
    Ok(())
}

/// A permanently stuck collector is bounded and cannot hang normal runtime drop.
#[test]
fn stuck_collector_does_not_block_runtime_shutdown() -> Result<()> {
    const CHILD_ENV: &str = "CODE_MODE_METRICS_SHUTDOWN_TEST_CHILD";
    if let Some(shutdown_marker) = std::env::var_os(CHILD_ENV) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            let calls = Arc::new(AtomicUsize::new(0));
            let collector_calls = calls.clone();
            let host =
                codex_code_mode_host::GrpcCodeModeHost::with_process_metrics(Arc::new(move || {
                    collector_calls.fetch_add(1, Ordering::SeqCst);
                    loop {
                        std::thread::park();
                    }
                }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let server = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
                tonic::transport::Server::builder()
                    .add_service(grpc::code_mode_host_server::CodeModeHostServer::new(host))
                    .serve_with_incoming(TcpListenerStream::new(listener))
                    .await
            }));
            let channel = Endpoint::from_shared(format!("http://{address}"))?
                .connect()
                .await?;
            let provider = GrpcCodeModeSessionProvider::with_channel(channel);
            assert!(provider.sample_process_metrics().await.is_err());
            let busy = provider.sample_process_metrics().await.unwrap_err();
            assert!(busy.contains("collector is busy"), "{busy}");
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            drop(server);
            Ok::<_, anyhow::Error>(())
        })?;
        drop(runtime);
        std::fs::write(shutdown_marker, "runtime dropped")?;
        return Ok(());
    }

    let directory = tempfile::tempdir()?;
    let shutdown_marker = directory.path().join("shutdown-completed");
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "process_metrics_tests::stuck_collector_does_not_block_runtime_shutdown",
            "--nocapture",
        ])
        .env(CHILD_ENV, &shutdown_marker)
        .spawn()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(/*secs*/ 10);
    loop {
        if let Some(status) = child.try_wait()? {
            assert!(status.success(), "metrics subprocess failed: {status}");
            assert_eq!(std::fs::read_to_string(shutdown_marker)?, "runtime dropped");
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("a stuck metrics collector blocked runtime shutdown");
        }
        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
    }
}
