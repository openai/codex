#![cfg(test)]
//! Deterministic child failures around channel selection and session recovery.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode::CodeModeProcessMetricsSample;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::FunctionCallOutputContentItem;
use codex_code_mode::GrpcCodeModeSessionProvider;
use core_test_support::code_mode_host::CodeModeHostRecorder;
use futures::future::try_join_all;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tonic::Status;
use tonic::body::Body;
use tower::ServiceExt;
use tower::service_fn;

use super::super::HostTransport;
use super::super::ProcessOwnedGrpcCodeModeSessionProvider;
use super::TEST_TIMEOUT;
use super::execute;

/// Failed initial HTTP/2 setup can retry admission, with exactly one replacement.
#[tokio::test]
async fn child_exit_during_initial_connection_retries_admission() -> Result<()> {
    let recorder = CodeModeHostRecorder::new()?;
    let directory = tempfile::tempdir()?;
    let wrapper = directory.path().join("exit-first");
    let attempts = directory.path().join("attempts");
    let marker = directory.path().join("failed-once");
    let program = recorder.program();
    let program = shlex::try_quote(program.to_str().context("host program")?)?;
    let quoted_attempts = shlex::try_quote(attempts.to_str().context("attempts")?)?;
    let marker = shlex::try_quote(marker.to_str().context("marker")?)?;
    codex_utils_cargo_bin::write_executable(
        &wrapper,
        &format!(
            "#!/bin/sh\nprintf x >> {quoted_attempts}\nif [ ! -e {marker} ]; then : > {marker}; exit 0; fi\nexec {program} \"$@\"\n"
        ),
    )?;
    let provider = ProcessOwnedGrpcCodeModeSessionProvider::with_host_program(wrapper);
    let session = timeout(TEST_TIMEOUT, provider.create_session())
        .await?
        .map_err(anyhow::Error::msg)?;
    assert_eq!(
        execute(&session, "text('ready');").await?,
        vec![FunctionCallOutputContentItem::InputText {
            text: "ready".to_string()
        }]
    );
    assert_eq!(std::fs::read_to_string(attempts)?, "xx");
    assert_eq!(recorder.invocations()?.len(), 1);
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

/// A selected generation cannot reconnect; concurrent sessions recover without replay.
#[tokio::test]
async fn child_exit_after_channel_selection_does_not_replay_execution() -> Result<()> {
    let recorder = CodeModeHostRecorder::new()?;
    let transport = Arc::new(HostTransport {
        host_program: recorder.program(),
        current: Arc::new(Mutex::new(None)),
    });
    let armed = Arc::new(AtomicBool::new(false));
    let selected = Arc::new(Notify::new());
    let dispatch = Arc::new(Semaphore::new(0));
    let provider = GrpcCodeModeSessionProvider::with_transport(service_fn({
        let armed = armed.clone();
        let selected = selected.clone();
        let dispatch = dispatch.clone();
        move |request: http::Request<Body>| {
            let transport = transport.clone();
            let armed = armed.clone();
            let selected = selected.clone();
            let dispatch = dispatch.clone();
            async move {
                // Exercise the production selector, pausing exactly between
                // selecting a live generation and dispatching to that channel.
                let channel = transport.channel(request.uri().path())?;
                if request.uri().path() == "/codex.code_mode.v1.CodeModeHost/Execute"
                    && armed.swap(false, Ordering::SeqCst)
                {
                    selected.notify_one();
                    dispatch.acquire().await.unwrap().forget();
                }
                channel
                    .oneshot(request)
                    .await
                    .map_err(|error| Status::from_error(Box::new(error)))
            }
        }
    }));
    let sessions = timeout(
        TEST_TIMEOUT,
        try_join_all((0..4).map(|_| provider.create_session())),
    )
    .await?
    .map_err(anyhow::Error::msg)?;
    let samples = Arc::new(AtomicUsize::new(0));
    let stop = CancellationToken::new();
    let sampler = tokio::spawn({
        let provider = provider.clone();
        let samples = samples.clone();
        let stop = stop.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_millis(/*millis*/ 5));
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    _ = interval.tick() => {
                        let _ = provider.sample_process_metrics().await;
                        samples.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        }
    });
    armed.store(true, Ordering::SeqCst);
    let pending = tokio::spawn({
        let session = sessions[0].clone();
        async move { execute(&session, "store('replayed', true);").await }
    });
    timeout(TEST_TIMEOUT, selected.notified()).await?;
    let pid = recorder.pid()?;
    assert_eq!(unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) }, 0);
    timeout(TEST_TIMEOUT, async {
        while unsafe {
            libc::kill(pid as libc::pid_t, /*sig*/ 0)
        } == 0
        {
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await
    .context("child was not reaped")?;
    let absent = provider.sample_process_metrics().await.unwrap_err();
    assert!(absent.contains("host is not running"), "{absent}");
    assert_eq!(recorder.invocations()?.len(), 1);
    dispatch.add_permits(1);
    assert!(timeout(TEST_TIMEOUT, pending).await??.is_err());
    assert_eq!(recorder.invocations()?.len(), 1);

    let recovery = try_join_all(sessions.iter().map(|session| async move {
        let error = execute(session, "store('replayed', true);")
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Stored values and running cells were lost"),
            "{error}"
        );
        assert_eq!(
            execute(session, "text(load('replayed') ?? 'clean');").await?,
            vec![FunctionCallOutputContentItem::InputText {
                text: "clean".to_string()
            }]
        );
        Ok::<_, anyhow::Error>(())
    }));
    timeout(TEST_TIMEOUT, recovery).await??;
    assert_eq!(
        provider
            .sample_process_metrics()
            .await
            .map_err(anyhow::Error::msg)?,
        CodeModeProcessMetricsSample::Unsupported
    );
    stop.cancel();
    timeout(TEST_TIMEOUT, sampler).await??;
    assert!(samples.load(Ordering::SeqCst) > 0);
    assert_eq!(recorder.invocations()?.len(), 2);
    for session in sessions {
        session.shutdown().await.map_err(anyhow::Error::msg)?;
    }
    Ok(())
}
