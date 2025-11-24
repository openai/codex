use std::convert::TryFrom;
use std::num::TryFromIntError;

use tracing::{subscriber::with_default, Level};
use tracing_attributes::instrument;
use tracing_mock::{expect, subscriber};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;
use tracing_test::block_on_future;

#[instrument(ret)]
fn ret() -> i32 {
    42
}

#[instrument(target = "my_target", ret)]
fn ret_with_target() -> i32 {
    42
}

#[test]
fn test() {
    let span = expect::span().named("ret");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(expect::field("return").with_value(&tracing::field::debug(42)))
                .at_level(Level::INFO),
        )
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, ret);
    handle.assert_finished();
}

#[test]
fn test_custom_target() {
    let filter: EnvFilter = "my_target=info".parse().expect("filter should parse");
    let span = expect::span()
        .named("ret_with_target")
        .with_target("my_target");

    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(expect::field("return").with_value(&tracing::field::debug(42)))
                .at_level(Level::INFO)
                .with_target("my_target"),
        )
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    let subscriber = subscriber.with(filter);

    with_default(subscriber, ret_with_target);
    handle.assert_finished();
}

#[instrument(level = "warn", ret)]
fn ret_warn() -> i32 {
    42
}

#[test]
fn test_warn() {
    let span = expect::span().named("ret_warn");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(expect::field("return").with_value(&tracing::field::debug(42)))
                .at_level(Level::WARN),
        )
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, ret_warn);
    handle.assert_finished();
}

#[instrument(ret)]
fn ret_mut(a: &mut i32) -> i32 {
    *a *= 2;
    tracing::info!(?a);
    *a
}

#[test]
fn test_mut() {
    let span = expect::span().named("ret_mut");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(expect::field("a").with_value(&tracing::field::display(2)))
                .at_level(Level::INFO),
        )
        .event(
            expect::event()
                .with_fields(expect::field("return").with_value(&tracing::field::debug(2)))
                .at_level(Level::INFO),
        )
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || ret_mut(&mut 1));
    handle.assert_finished();
}

#[instrument(ret)]
async fn ret_async() -> i32 {
    42
}

#[test]
fn test_async() {
    let span = expect::span().named("ret_async");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(expect::field("return").with_value(&tracing::field::debug(42)))
                .at_level(Level::INFO),
        )
        .exit(span.clone())
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || block_on_future(async { ret_async().await }));
    handle.assert_finished();
}

#[instrument(ret)]
fn ret_impl_type() -> impl Copy {
    42
}

#[test]
fn test_impl_type() {
    let span = expect::span().named("ret_impl_type");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(expect::field("return").with_value(&tracing::field::debug(42)))
                .at_level(Level::INFO),
        )
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, ret_impl_type);
    handle.assert_finished();
}

#[instrument(ret(Display))]
fn ret_display() -> i32 {
    42
}

#[test]
fn test_dbg() {
    let span = expect::span().named("ret_display");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(expect::field("return").with_value(&tracing::field::display(42)))
                .at_level(Level::INFO),
        )
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, ret_display);
    handle.assert_finished();
}

#[instrument(err, ret)]
fn ret_and_err() -> Result<u8, TryFromIntError> {
    u8::try_from(1234)
}

#[test]
fn test_ret_and_err() {
    let span = expect::span().named("ret_and_err");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(
                    expect::field("error")
                        .with_value(&tracing::field::display(u8::try_from(1234).unwrap_err()))
                        .only(),
                )
                .at_level(Level::ERROR),
        )
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || ret_and_err().ok());
    handle.assert_finished();
}

#[instrument(err, ret)]
fn ret_and_ok() -> Result<u8, TryFromIntError> {
    u8::try_from(123)
}

#[test]
fn test_ret_and_ok() {
    let span = expect::span().named("ret_and_ok");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(
                    expect::field("return")
                        .with_value(&tracing::field::debug(u8::try_from(123).unwrap()))
                        .only(),
                )
                .at_level(Level::INFO),
        )
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || ret_and_ok().ok());
    handle.assert_finished();
}

#[instrument(level = "warn", ret(level = "info"))]
fn ret_warn_info() -> i32 {
    42
}

#[test]
fn test_warn_info() {
    let span = expect::span().named("ret_warn_info").at_level(Level::WARN);
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(expect::field("return").with_value(&tracing::field::debug(42)))
                .at_level(Level::INFO),
        )
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, ret_warn_info);
    handle.assert_finished();
}

#[instrument(ret(level = "warn", Debug))]
fn ret_dbg_warn() -> i32 {
    42
}

#[test]
fn test_dbg_warn() {
    let span = expect::span().named("ret_dbg_warn").at_level(Level::INFO);
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .event(
            expect::event()
                .with_fields(expect::field("return").with_value(&tracing::field::debug(42)))
                .at_level(Level::WARN),
        )
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, ret_dbg_warn);
    handle.assert_finished();
}
