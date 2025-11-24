use tracing::Level;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn prefixed_span_macros() {
    tracing::span!(Level::DEBUG, "foo");
    tracing::trace_span!("foo");
    tracing::debug_span!("foo");
    tracing::info_span!("foo");
    tracing::warn_span!("foo");
    tracing::error_span!("foo");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn prefixed_event_macros() {
    tracing::event!(Level::DEBUG, "foo");
    tracing::trace!("foo");
    tracing::debug!("foo");
    tracing::info!("foo");
    tracing::warn!("foo");
    tracing::error!("foo");
}
