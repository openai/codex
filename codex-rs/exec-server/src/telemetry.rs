pub fn runtime_span() -> tracing::Span {
    tracing::info_span!("codex.exec_server", otel.kind = "internal")
}
