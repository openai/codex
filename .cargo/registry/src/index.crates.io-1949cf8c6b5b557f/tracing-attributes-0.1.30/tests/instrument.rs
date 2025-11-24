use tracing::subscriber::with_default;
use tracing::Level;
use tracing_attributes::instrument;
use tracing_mock::*;

// Reproduces a compile error when an instrumented function body contains inner
// attributes (https://github.com/tokio-rs/tracing/issues/2294).
#[deny(unused_variables)]
#[allow(dead_code, clippy::mixed_attributes_style)]
#[instrument]
fn repro_2294() {
    #![allow(unused_variables)]
    let i = 42;
}

#[test]
fn override_everything() {
    #[instrument(target = "my_target", level = "debug")]
    fn my_fn() {}

    #[instrument(level = Level::DEBUG, target = "my_target")]
    fn my_other_fn() {}

    let span = expect::span()
        .named("my_fn")
        .at_level(Level::DEBUG)
        .with_target("my_target");
    let span2 = expect::span()
        .named("my_other_fn")
        .at_level(Level::DEBUG)
        .with_target("my_target");
    let (subscriber, handle) = subscriber::mock()
        .new_span(span.clone())
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .new_span(span2.clone())
        .enter(span2.clone())
        .exit(span2.clone())
        .drop_span(span2)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        my_fn();
        my_other_fn();
    });

    handle.assert_finished();
}

#[test]
fn fields() {
    #[instrument(target = "my_target", level = "debug")]
    fn my_fn(arg1: usize, arg2: bool, arg3: String) {}

    let span = expect::span()
        .named("my_fn")
        .at_level(Level::DEBUG)
        .with_target("my_target");

    let span2 = expect::span()
        .named("my_fn")
        .at_level(Level::DEBUG)
        .with_target("my_target");
    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone().with_fields(
                expect::field("arg1")
                    .with_value(&2usize)
                    .and(expect::field("arg2").with_value(&false))
                    .and(expect::field("arg3").with_value(&"Cool".to_string()))
                    .only(),
            ),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .new_span(
            span2.clone().with_fields(
                expect::field("arg1")
                    .with_value(&3usize)
                    .and(expect::field("arg2").with_value(&true))
                    .and(expect::field("arg3").with_value(&"Still Cool".to_string()))
                    .only(),
            ),
        )
        .enter(span2.clone())
        .exit(span2.clone())
        .drop_span(span2)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        my_fn(2, false, "Cool".to_string());
        my_fn(3, true, "Still Cool".to_string());
    });

    handle.assert_finished();
}

#[test]
fn skip() {
    struct UnDebug();

    #[instrument(target = "my_target", level = "debug", skip(_arg2, _arg3))]
    fn my_fn(arg1: usize, _arg2: UnDebug, _arg3: UnDebug) {}

    #[instrument(target = "my_target", level = "debug", skip_all)]
    fn my_fn2(_arg1: usize, _arg2: UnDebug, _arg3: UnDebug) {}

    let span = expect::span()
        .named("my_fn")
        .at_level(Level::DEBUG)
        .with_target("my_target");

    let span2 = expect::span()
        .named("my_fn")
        .at_level(Level::DEBUG)
        .with_target("my_target");

    let span3 = expect::span()
        .named("my_fn2")
        .at_level(Level::DEBUG)
        .with_target("my_target");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone()
                .with_fields(expect::field("arg1").with_value(&2usize).only()),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .new_span(
            span2
                .clone()
                .with_fields(expect::field("arg1").with_value(&3usize).only()),
        )
        .enter(span2.clone())
        .exit(span2.clone())
        .drop_span(span2)
        .new_span(span3.clone())
        .enter(span3.clone())
        .exit(span3.clone())
        .drop_span(span3)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        my_fn(2, UnDebug(), UnDebug());
        my_fn(3, UnDebug(), UnDebug());
        my_fn2(2, UnDebug(), UnDebug());
    });

    handle.assert_finished();
}

#[test]
fn generics() {
    #[derive(Debug)]
    struct Foo;

    #[instrument]
    fn my_fn<S, T: std::fmt::Debug>(arg1: S, arg2: T)
    where
        S: std::fmt::Debug,
    {
    }

    let span = expect::span().named("my_fn");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone().with_fields(
                expect::field("arg1")
                    .with_value(&format_args!("Foo"))
                    .and(expect::field("arg2").with_value(&format_args!("false"))),
            ),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        my_fn(Foo, false);
    });

    handle.assert_finished();
}

#[test]
fn methods() {
    #[derive(Debug)]
    struct Foo;

    impl Foo {
        #[instrument]
        fn my_fn(&self, arg1: usize) {}
    }

    let span = expect::span().named("my_fn");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone().with_fields(
                expect::field("self")
                    .with_value(&format_args!("Foo"))
                    .and(expect::field("arg1").with_value(&42usize)),
            ),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        let foo = Foo;
        foo.my_fn(42);
    });

    handle.assert_finished();
}

#[test]
fn impl_trait_return_type() {
    #[instrument]
    fn returns_impl_trait(x: usize) -> impl Iterator<Item = usize> {
        0..x
    }

    let span = expect::span().named("returns_impl_trait");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone()
                .with_fields(expect::field("x").with_value(&10usize).only()),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        for _ in returns_impl_trait(10) {
            // nop
        }
    });

    handle.assert_finished();
}

#[test]
fn name_ident() {
    const MY_NAME: &str = "my_name";
    #[instrument(name = MY_NAME)]
    fn name() {}

    let span_name = expect::span().named(MY_NAME);

    let (subscriber, handle) = subscriber::mock()
        .new_span(span_name.clone())
        .enter(span_name.clone())
        .exit(span_name.clone())
        .drop_span(span_name)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        name();
    });

    handle.assert_finished();
}

#[test]
fn target_ident() {
    const MY_TARGET: &str = "my_target";

    #[instrument(target = MY_TARGET)]
    fn target() {}

    let span_target = expect::span().named("target").with_target(MY_TARGET);

    let (subscriber, handle) = subscriber::mock()
        .new_span(span_target.clone())
        .enter(span_target.clone())
        .exit(span_target.clone())
        .drop_span(span_target)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        target();
    });

    handle.assert_finished();
}

#[test]
fn target_name_ident() {
    const MY_NAME: &str = "my_name";
    const MY_TARGET: &str = "my_target";

    #[instrument(target = MY_TARGET, name = MY_NAME)]
    fn name_target() {}

    let span_name_target = expect::span().named(MY_NAME).with_target(MY_TARGET);

    let (subscriber, handle) = subscriber::mock()
        .new_span(span_name_target.clone())
        .enter(span_name_target.clone())
        .exit(span_name_target.clone())
        .drop_span(span_name_target)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        name_target();
    });

    handle.assert_finished();
}

#[test]
fn user_tracing_module() {
    use ::tracing::field::Empty;

    // Reproduces https://github.com/tokio-rs/tracing/issues/3119
    #[instrument(fields(f = Empty))]
    #[allow(dead_code)]
    fn my_fn() {
        assert_eq!("test", tracing::my_other_fn());
    }

    mod tracing {
        #[allow(dead_code)]
        pub fn my_other_fn() -> &'static str {
            "test"
        }
    }
}
