use super::*;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use codex_analytics::AnalyticsEventsClient;
use codex_core::config::ConfigBuilder;
use codex_feedback::CodexFeedback;
use codex_state::LogWriteFailureReporter;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn log_write_warning_is_broadcast_once() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    for feedback_enabled in [true, false] {
        config.feedback_enabled = feedback_enabled;
        let feedback = CodexFeedback::new();
        let (tx, mut rx) = mpsc::channel(/*buffer*/ 4);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            tx,
            AnalyticsEventsClient::disabled(),
        ));
        let reporter = LogWriteWarningReporter::new(feedback.clone(), &outgoing, &config);
        reporter.report_failure("SQLite flush failed");
        let OutgoingEnvelope::Broadcast {
            message: OutgoingMessage::AppServerNotification(envelope),
        } = rx.try_recv()?
        else {
            panic!("expected a broadcast notification");
        };
        let expected = if feedback_enabled {
            "Codex couldn't save diagnostic logs to its local database. Use /feedback with logs included before closing Codex, or run `codex doctor` for diagnostics."
        } else {
            "Codex couldn't save diagnostic logs to its local database. Run `codex doctor` for diagnostics."
        };
        let ServerNotification::Warning(notification) = envelope.notification else {
            panic!("expected a warning");
        };
        assert_eq!(
            notification,
            WarningNotification {
                thread_id: None,
                message: expected.to_string(),
            }
        );
        reporter.report_failure("another SQLite flush failed");
        assert!(rx.try_recv().is_err());
        assert_eq!(
            feedback
                .snapshot(/*session_id*/ None)
                .log_attachment(/*logs_override*/ None)
                .buffer,
            b"SQLite flush failedanother SQLite flush failed"
        );
    }
    Ok(())
}

#[tokio::test]
async fn log_write_warning_keeps_feedback_when_sender_is_gone() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    let feedback = CodexFeedback::new();
    let (tx, mut rx) = mpsc::channel(/*buffer*/ 4);
    let outgoing = Arc::new(OutgoingMessageSender::new(
        tx,
        AnalyticsEventsClient::disabled(),
    ));
    let reporter = LogWriteWarningReporter::new(feedback.clone(), &outgoing, &config);
    drop(outgoing);
    reporter.report_failure("SQLite flush failed");
    assert_eq!(
        feedback
            .snapshot(/*session_id*/ None)
            .log_attachment(/*logs_override*/ None)
            .buffer,
        b"SQLite flush failed"
    );
    assert!(reporter.outgoing.upgrade().is_none());
    reporter.report_failure("second failure");
    assert_eq!(
        feedback
            .snapshot(/*session_id*/ None)
            .log_attachment(/*logs_override*/ None)
            .buffer,
        b"SQLite flush failedsecond failure"
    );
    assert!(rx.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn log_write_warning_reports_failed_sqlite_flush() -> anyhow::Result<()> {
    use tracing_subscriber::layer::SubscriberExt;

    for attach_after_failure in [false, true] {
        let home = tempfile::tempdir()?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        let feedback = CodexFeedback::new();
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_config().clone(),
            "test-provider".to_string(),
        )
        .await?;
        let (tx, mut rx) = mpsc::channel(/*buffer*/ 4);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            tx,
            AnalyticsEventsClient::disabled(),
        ));
        let reporter = LogWriteWarningReporter::new(feedback.clone(), &outgoing, &config);
        let initial_reporter: Arc<dyn LogWriteFailureReporter> = if attach_after_failure {
            Arc::new(feedback.clone())
        } else {
            reporter.clone()
        };
        let layer = codex_state::log_db::start(runtime.clone(), initial_reporter);
        runtime.close().await;
        tracing::subscriber::with_default(
            tracing_subscriber::registry().with(layer.clone()),
            || {
                tracing::info!("first failed log write");
            },
        );
        layer.flush().await;
        assert!(layer.has_write_failure());
        if attach_after_failure {
            layer.set_failure_reporter(reporter.clone());
            if layer.has_write_failure() {
                reporter.notify_failure();
            }
        }

        assert!(matches!(
            rx.try_recv(),
            Ok(OutgoingEnvelope::Broadcast {
                message: OutgoingMessage::AppServerNotification(_),
                ..
            })
        ));
        tracing::subscriber::with_default(
            tracing_subscriber::registry().with(layer.clone()),
            || {
                tracing::info!("second failed log write");
            },
        );
        layer.flush().await;
        assert!(rx.try_recv().is_err());
        assert_eq!(
            String::from_utf8(
                feedback
                    .snapshot(/*session_id*/ None)
                    .log_attachment(/*logs_override*/ None)
                    .buffer
            )?
            .matches("failed to flush logs to SQLite")
            .count(),
            2
        );
    }
    Ok(())
}
