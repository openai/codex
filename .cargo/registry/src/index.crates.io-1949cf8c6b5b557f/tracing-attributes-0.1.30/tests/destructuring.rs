use tracing::subscriber::with_default;
use tracing_attributes::instrument;
use tracing_mock::*;

#[test]
fn destructure_tuples() {
    #[instrument]
    fn my_fn((arg1, arg2): (usize, usize)) {}

    let span = expect::span().named("my_fn");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone().with_fields(
                expect::field("arg1")
                    .with_value(&format_args!("1"))
                    .and(expect::field("arg2").with_value(&format_args!("2")))
                    .only(),
            ),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        my_fn((1, 2));
    });

    handle.assert_finished();
}

#[test]
fn destructure_nested_tuples() {
    #[instrument]
    fn my_fn(((arg1, arg2), (arg3, arg4)): ((usize, usize), (usize, usize))) {}

    let span = expect::span().named("my_fn");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone().with_fields(
                expect::field("arg1")
                    .with_value(&format_args!("1"))
                    .and(expect::field("arg2").with_value(&format_args!("2")))
                    .and(expect::field("arg3").with_value(&format_args!("3")))
                    .and(expect::field("arg4").with_value(&format_args!("4")))
                    .only(),
            ),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        my_fn(((1, 2), (3, 4)));
    });

    handle.assert_finished();
}

#[test]
fn destructure_refs() {
    #[instrument]
    fn my_fn(&arg1: &usize) {}

    let span = expect::span().named("my_fn");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone()
                .with_fields(expect::field("arg1").with_value(&1usize).only()),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        my_fn(&1);
    });

    handle.assert_finished();
}

#[test]
fn destructure_tuple_structs() {
    struct Foo(usize, usize);

    #[instrument]
    fn my_fn(Foo(arg1, arg2): Foo) {}

    let span = expect::span().named("my_fn");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone().with_fields(
                expect::field("arg1")
                    .with_value(&format_args!("1"))
                    .and(expect::field("arg2").with_value(&format_args!("2")))
                    .only(),
            ),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        my_fn(Foo(1, 2));
    });

    handle.assert_finished();
}

#[test]
fn destructure_structs() {
    struct Foo {
        bar: usize,
        baz: usize,
    }

    #[instrument]
    fn my_fn(
        Foo {
            bar: arg1,
            baz: arg2,
        }: Foo,
    ) {
        let _ = (arg1, arg2);
    }

    let span = expect::span().named("my_fn");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone().with_fields(
                expect::field("arg1")
                    .with_value(&format_args!("1"))
                    .and(expect::field("arg2").with_value(&format_args!("2")))
                    .only(),
            ),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        my_fn(Foo { bar: 1, baz: 2 });
    });

    handle.assert_finished();
}

#[test]
fn destructure_everything() {
    struct Foo {
        bar: Bar,
        baz: (usize, usize),
        qux: NoDebug,
    }
    struct Bar((usize, usize));
    struct NoDebug;

    #[instrument]
    fn my_fn(
        &Foo {
            bar: Bar((arg1, arg2)),
            baz: (arg3, arg4),
            ..
        }: &Foo,
    ) {
        let _ = (arg1, arg2, arg3, arg4);
    }

    let span = expect::span().named("my_fn");

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            span.clone().with_fields(
                expect::field("arg1")
                    .with_value(&format_args!("1"))
                    .and(expect::field("arg2").with_value(&format_args!("2")))
                    .and(expect::field("arg3").with_value(&format_args!("3")))
                    .and(expect::field("arg4").with_value(&format_args!("4")))
                    .only(),
            ),
        )
        .enter(span.clone())
        .exit(span.clone())
        .drop_span(span)
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        let foo = Foo {
            bar: Bar((1, 2)),
            baz: (3, 4),
            qux: NoDebug,
        };
        let _ = foo.qux; // to eliminate unused field warning
        my_fn(&foo);
    });

    handle.assert_finished();
}
