#![cfg(feature = "std")]

use tracing_mock::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn spans_dont_leak() {
    fn do_span() {
        let span = tracing::debug_span!("alice");
        let _e = span.enter();
    }

    let (subscriber, handle) = subscriber::mock()
        .named("spans/subscriber1")
        .with_filter(|_| false)
        .only()
        .run_with_handle();

    let _guard = tracing::subscriber::set_default(subscriber);

    do_span();

    let alice = expect::span().named("alice");
    let (subscriber2, handle2) = subscriber::mock()
        .named("spans/subscriber2")
        .with_filter(|_| true)
        .new_span(alice.clone())
        .enter(alice.clone())
        .exit(alice.clone())
        .drop_span(alice)
        .only()
        .run_with_handle();

    tracing::subscriber::with_default(subscriber2, || {
        println!("--- subscriber 2 is default ---");
        do_span()
    });

    println!("--- subscriber 1 is default ---");
    do_span();

    handle.assert_finished();
    handle2.assert_finished();
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn events_dont_leak() {
    fn do_event() {
        tracing::debug!("alice");
    }

    let (subscriber, handle) = subscriber::mock()
        .named("events/subscriber1")
        .with_filter(|_| false)
        .only()
        .run_with_handle();

    let _guard = tracing::subscriber::set_default(subscriber);

    do_event();

    let (subscriber2, handle2) = subscriber::mock()
        .named("events/subscriber2")
        .with_filter(|_| true)
        .event(expect::event())
        .only()
        .run_with_handle();

    tracing::subscriber::with_default(subscriber2, || {
        println!("--- subscriber 2 is default ---");
        do_event()
    });

    println!("--- subscriber 1 is default ---");

    do_event();

    handle.assert_finished();
    handle2.assert_finished();
}
