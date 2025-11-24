use std::thread;
use std::time::Duration;

use sentry::protocol::Request;

// cargo run --example performance-demo
fn main() {
    let _sentry = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        traces_sample_rate: 1.0,
        debug: true,
        ..Default::default()
    });

    let transaction =
        sentry::start_transaction(sentry::TransactionContext::new("transaction", "root span"));
    let tx_request = Request {
        url: Some("https://honk.beep".parse().unwrap()),
        method: Some("GET".to_string()),
        ..Request::default()
    };
    transaction.set_request(tx_request);
    sentry::configure_scope(|scope| scope.set_span(Some(transaction.clone().into())));

    main_span1();

    thread::sleep(Duration::from_millis(100));

    transaction.finish();
    sentry::configure_scope(|scope| scope.set_span(None));
}

fn main_span1() {
    wrap_in_span("span1", "", || {
        thread::sleep(Duration::from_millis(50));

        let transaction_ctx = sentry::TransactionContext::continue_from_span(
            "background transaction",
            "root span",
            sentry::configure_scope(|scope| scope.get_span()),
        );
        thread::spawn(move || {
            let transaction = sentry::start_transaction(transaction_ctx);
            sentry::configure_scope(|scope| scope.set_span(Some(transaction.clone().into())));

            thread::sleep(Duration::from_millis(50));

            thread_span1();

            transaction.finish();
            sentry::configure_scope(|scope| scope.set_span(None));
        });
        thread::sleep(Duration::from_millis(100));

        main_span2()
    });
}

fn thread_span1() {
    wrap_in_span("span1", "", || {
        thread::sleep(Duration::from_millis(200));
    })
}

fn main_span2() {
    wrap_in_span("span2", "", || {
        sentry::capture_message(
            "A message that should have a trace context",
            sentry::Level::Info,
        );
        thread::sleep(Duration::from_millis(200));
    })
}

fn wrap_in_span<F, R>(op: &str, description: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let parent = sentry::configure_scope(|scope| scope.get_span());
    let span1: sentry::TransactionOrSpan = match &parent {
        Some(parent) => parent.start_child(op, description).into(),
        None => {
            let ctx = sentry::TransactionContext::new(description, op);
            sentry::start_transaction(ctx).into()
        }
    };
    let span_request = Request {
        url: Some("https://beep.beep".parse().unwrap()),
        method: Some("GET".to_string()),
        ..Request::default()
    };
    span1.set_request(span_request);
    sentry::configure_scope(|scope| scope.set_span(Some(span1.clone())));

    let rv = f();

    span1.finish();
    sentry::configure_scope(|scope| scope.set_span(parent));

    rv
}
