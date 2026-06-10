use std::time::Duration;
use std::time::Instant;

use tracing::Span;
use tracing::info;
use tracing::warn;

use crate::ExecServerError;
use crate::ExecServerTelemetry;
use crate::telemetry::RemoteWebSocketMetricGuard;

macro_rules! emit_remote_otel_event {
    ($level:ident, $event_name:literal, $($fields:tt)*) => {{
        let span = tracing::info_span!(
            "codex.exec_server.remote_event",
            otel.kind = "internal",
            otel.name = $event_name,
        );
        span.in_scope(|| {
            tracing::event!(
                target: "codex_otel.log_only",
                tracing::Level::$level,
                event.name = $event_name,
                $($fields)*
            );
            tracing::event!(
                target: "codex_otel.trace_safe",
                tracing::Level::$level,
                event.name = $event_name,
                $($fields)*
            );
        });
    }};
}

pub(crate) fn registration_span() -> Span {
    tracing::info_span!(
        "codex.exec_server.remote.register",
        otel.kind = "client",
        otel.name = "codex.exec_server.remote.register",
        result = tracing::field::Empty,
    )
}

pub(crate) fn registration_succeeded(
    telemetry: &ExecServerTelemetry,
    span: Span,
    started_at: Instant,
    environment_id: &str,
) {
    span.record("result", "success");
    telemetry.remote_registration_completed("success", started_at.elapsed());
    drop(span);

    eprintln!(
        "codex exec-server remote environment registered with environment_id {environment_id}"
    );
    info!(
        environment_id,
        "codex exec-server remote environment registered"
    );
    emit_remote_otel_event!(
        INFO,
        "codex.exec_server.remote_environment_registered",
        "codex exec-server remote environment registered"
    );
}

pub(crate) fn registration_failed(
    telemetry: &ExecServerTelemetry,
    span: Span,
    started_at: Instant,
    err: &ExecServerError,
) {
    span.record("result", "error");
    telemetry.remote_registration_completed("error", started_at.elapsed());
    drop(span);

    warn!(error = %err, "failed to register remote exec-server environment");
    emit_remote_otel_event!(
        WARN,
        "codex.exec_server.remote_environment_registration_failed",
        "failed to register remote exec-server environment"
    );
}

pub(crate) fn websocket_connect_span(attempt: u32) -> Span {
    tracing::info_span!(
        "codex.exec_server.remote.websocket.connect",
        otel.kind = "client",
        otel.name = "codex.exec_server.remote.websocket.connect",
        attempt,
        result = tracing::field::Empty,
    )
}

pub(crate) fn websocket_connected(
    telemetry: &ExecServerTelemetry,
    span: Span,
    started_at: Instant,
    attempt: u32,
) -> RemoteWebSocketMetricGuard {
    span.record("result", "success");
    telemetry.remote_websocket_connect_completed("success", started_at.elapsed());
    drop(span);

    info!(attempt, "connected remote exec-server websocket");
    emit_remote_otel_event!(
        INFO,
        "codex.exec_server.remote_websocket_connected",
        attempt,
        "connected remote exec-server websocket"
    );
    telemetry.remote_websocket_connected()
}

pub(crate) fn websocket_connect_failed(
    telemetry: &ExecServerTelemetry,
    span: Span,
    started_at: Instant,
    attempt: u32,
    err: &tokio_tungstenite::tungstenite::Error,
) {
    span.record("result", "error");
    telemetry.remote_websocket_connect_completed("error", started_at.elapsed());
    drop(span);

    warn!(
        attempt,
        error = %err,
        "failed to connect remote exec-server websocket"
    );
    emit_remote_otel_event!(
        WARN,
        "codex.exec_server.remote_websocket_connect_failed",
        attempt,
        "failed to connect remote exec-server websocket"
    );
    telemetry.remote_websocket_reconnect("connect_failed");
}

pub(crate) fn websocket_disconnected(telemetry: &ExecServerTelemetry, attempt: u32) {
    telemetry.remote_websocket_reconnect("disconnected");
    warn!(
        attempt,
        "remote exec-server websocket disconnected; retrying"
    );
    emit_remote_otel_event!(
        WARN,
        "codex.exec_server.remote_websocket_disconnected",
        attempt,
        "remote exec-server websocket disconnected; retrying"
    );
}

pub(crate) fn websocket_retrying(attempt: u32, backoff: Duration) {
    let backoff_ms = backoff.as_millis();
    info!(attempt, backoff_ms, "retrying remote exec-server websocket");
    emit_remote_otel_event!(
        INFO,
        "codex.exec_server.remote_websocket_retrying",
        attempt,
        backoff_ms,
        "retrying remote exec-server websocket"
    );
}
