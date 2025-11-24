//! Sentry Scope Benchmarks
//!
//! Run the benchmarks with:
//!
//! ```text
//! $ cargo bench -p sentry-core
//! ```
//!
//! We have the following tests:
//! * [`scope_with_breadcrumbs`]
//! * [`scope_with_tags`]
//!
//! We do test the following permutations:
//! * No active Hub is bound, meaning most API functions will just noop
//! * Doing scope manipulation with an active Hub, but *not* capturing any messages
//! * Doing scope manipulation with an active Hub, and capturing messages, discarding
//!   them in the transport layer.
//!
//! # Testing the minimal API
//! Due to our circular dev-dependency on `sentry`, we will *always* run with the
//! `client` feature. To test without it, one needs to comment the circular dependency
//! before running the benchmark.

use std::ops::Range;
#[cfg(feature = "client")]
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};
use sentry::protocol::Breadcrumb;
#[cfg(not(feature = "client"))]
use sentry_core as sentry;

/// Tests Scopes with Breadcrumbs
///
/// This uses the [`sentry::add_breadcrumb`] API in *callback mode*, which means
/// it is essentially a noop when the current Hub is inactive.
fn scope_with_breadcrumbs(capture: bool) {
    add_breadcrumbs(0..50);

    if capture {
        sentry::capture_message("capturing on outer scope", sentry::Level::Info);
    }

    sentry::with_scope(
        |_| (),
        || {
            // 50 + 70 exceeds the default max_breadcrumbs of 100
            add_breadcrumbs(50..120);

            if capture {
                sentry::capture_message("capturing within a nested scope", sentry::Level::Info);
            }
        },
    );

    sentry::configure_scope(|scope| scope.clear());
}

/// This does multiple nested [`sentry::with_scope`] calls that manipulates breadcrumbs
fn outer_scope_with_breadcrumbs() {
    sentry::with_scope(
        |_| (),
        || {
            add_breadcrumbs(20..50);
            sentry::with_scope(
                |_| (),
                || {
                    add_breadcrumbs(50..80);
                },
            )
        },
    )
}

/// Tests Scopes with Tags
///
/// This uses the [`sentry::Scope::set_tag`] function to define, and then overwrite/extend
/// the set of tags.
fn scope_with_tags(capture: bool) {
    sentry::configure_scope(|scope| add_tags(scope, 0..20));

    if capture {
        sentry::capture_message("capturing on outer scope", sentry::Level::Info);
    }

    sentry::with_scope(
        |scope| {
            // since this is a hashmap, we basically overwrite 10, and add 10 new tags
            add_tags(scope, 10..30)
        },
        || {
            if capture {
                sentry::capture_message("capturing within a nested scope", sentry::Level::Info);
            }
        },
    );

    sentry::configure_scope(|scope| scope.clear());
}

/// This does multiple nested [`sentry::with_scope`] calls that manipulates tags
fn outer_scope_with_tags() {
    sentry::with_scope(
        |scope| add_tags(scope, 20..22),
        || sentry::with_scope(|scope| add_tags(scope, 22..24), || {}),
    )
}

/// Adds a bunch of breadcrumbs
fn add_breadcrumbs(range: Range<usize>) {
    for i in range {
        sentry::add_breadcrumb(|| Breadcrumb {
            message: Some(format!("Breadcrumb {}", i)),
            ..Default::default()
        });
    }
}

/// Adds a bunch of tags
fn add_tags(scope: &mut sentry::Scope, range: Range<usize>) {
    for i in range {
        scope.set_tag(&format!("tag {}", i), format!("tag value {}", i));
    }
}

/// Returns a new *active* [`sentry::Hub`] which discards Events in the Transport.
#[cfg(feature = "client")]
fn discarding_hub() -> sentry::Hub {
    struct NoopTransport;

    impl sentry::Transport for NoopTransport {
        fn send_envelope(&self, envelope: sentry::Envelope) {
            drop(envelope)
        }
    }

    let client = Arc::new(sentry::Client::from(sentry::ClientOptions {
        dsn: Some("https://public@sentry.invalid/1".parse().unwrap()),
        // lol, this double arcing -_-
        transport: Some(Arc::new(Arc::new(NoopTransport))),
        // before_send: Some(Arc::new(|_| None)),
        ..Default::default()
    }));
    let scope = Arc::new(sentry::Scope::default());
    sentry::Hub::new(Some(client), scope)
}

fn scope_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("scoped-tags");

    group.bench_function("no-client", |b| b.iter(|| scope_with_tags(true)));
    #[cfg(feature = "client")]
    {
        group.bench_function("with-client", |b| {
            let hub = Arc::new(discarding_hub());
            sentry::Hub::run(hub, || b.iter(|| scope_with_tags(false)))
        });
        group.bench_function("dropping-client", |b| {
            let hub = Arc::new(discarding_hub());
            sentry::Hub::run(hub, || b.iter(|| scope_with_tags(true)))
        });
        group.bench_function("outer-scope", |b| {
            let hub = Arc::new(discarding_hub());
            sentry::configure_scope(|scope| add_tags(scope, 0..20));
            sentry::Hub::run(hub, || b.iter(outer_scope_with_tags))
        });
    }

    group.finish();

    let mut group = c.benchmark_group("scoped-breadcrumbs");

    group.bench_function("no-client", |b| b.iter(|| scope_with_breadcrumbs(true)));
    #[cfg(feature = "client")]
    {
        group.bench_function("with-client", |b| {
            let hub = Arc::new(discarding_hub());
            sentry::Hub::run(hub, || b.iter(|| scope_with_breadcrumbs(false)))
        });
        group.bench_function("dropping-client", |b| {
            let hub = Arc::new(discarding_hub());
            sentry::Hub::run(hub, || b.iter(|| scope_with_breadcrumbs(true)))
        });
        group.bench_function("outer-scope", |b| {
            let hub = Arc::new(discarding_hub());
            add_breadcrumbs(0..20);
            sentry::Hub::run(hub, || b.iter(outer_scope_with_breadcrumbs))
        });
    }

    group.finish();
}

criterion_group!(benches, scope_benchmark);
criterion_main!(benches);
