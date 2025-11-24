#![cfg(feature = "std")]

use tracing_mock::subscriber;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn no_subscriber_disables_global() {
    // Reproduces https://github.com/tokio-rs/tracing/issues/1999
    let (subscriber, handle) = subscriber::mock().only().run_with_handle();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting global default must succeed");
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
        tracing::info!("this should not be recorded");
    });
    handle.assert_finished();
}
