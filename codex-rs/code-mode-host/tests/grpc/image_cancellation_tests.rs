//! Cancellation of reentrant image getters through the standalone host.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn cancellation_stops_reentrant_image_getter() -> Result<()> {
    // Force reentry and JSON fallback at a shallow depth, independent of stack limits.
    let source = r#"
let calls = 0;
const deadline = Date.now() + 60000;
const value = {
    get image_url() {
        switch (++calls) {
            case 1: image(value); return "";
            case 2: throw new Error("enter fallback");
            default:
                notify("started");
                while (Date.now() < deadline) {}
                return "";
        }
    }
};
try { image(value); } catch { text(calls); }
"#;
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let provider = GrpcCodeModeSessionProvider::new(host.endpoint);
    let session = provider
        .create_session()
        .await
        .map_err(anyhow::Error::msg)?;
    let other_session = provider
        .create_session()
        .await
        .map_err(anyhow::Error::msg)?;
    let other_response = execute(
        &other_session,
        request(r#"store("alive", "yes"); text("yes");"#),
        Arc::new(NoopCodeModeSessionDelegate),
    )
    .await?;
    assert_eq!(
        other_response,
        text_response("1", "yes", other_response.code_mode_host_duration())
    );

    let delegate = Arc::new(RecordingDelegate::default());
    let mut pending = request(source);
    pending.yield_time_ms = Some(60_000);
    let started = session
        .execute(pending, delegate.clone(), /*preempt*/ None)
        .await
        .map_err(anyhow::Error::msg)?;
    let id = started.cell_id.clone();
    // The second image call enters JSON fallback; wait until its getter is running.
    timeout(TEST_TIMEOUT, delegate.notification_delivered.notified())
        .await
        .context("getter did not signal startup")?;

    let result = timeout(Duration::from_secs(1), session.terminate(id.clone()))
        .await
        .context("workload did not stop within the cancellation deadline")?
        .map_err(anyhow::Error::msg)?;
    // The getter would otherwise run for 60 seconds and emit its counter.
    assert_eq!(
        result,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            code_mode_host_duration: result.code_mode_host_duration(),
            cell_id: id.clone(),
            content_items: Vec::new(),
        })
    );
    let initial = timeout(TEST_TIMEOUT, started.initial_response())
        .await
        .context("initial termination response did not arrive")?
        .map_err(anyhow::Error::msg)?;
    assert_eq!(
        initial,
        RuntimeResponse::Terminated {
            code_mode_host_duration: initial.code_mode_host_duration(),
            cell_id: id,
            content_items: Vec::new(),
        }
    );
    let other_response = execute(
        &other_session,
        request(r#"text(load("alive"));"#),
        Arc::new(NoopCodeModeSessionDelegate),
    )
    .await?;
    assert_eq!(
        other_response,
        text_response("2", "yes", other_response.code_mode_host_duration())
    );
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    other_session.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}
