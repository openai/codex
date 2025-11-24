use tracing::subscriber::with_default;
use tracing_attributes::instrument;
use tracing_mock::{expect, span::NewSpan, subscriber};

#[instrument(fields(foo = "bar", dsa = true, num = 1))]
fn fn_no_param() {}

#[instrument(fields(foo = "bar"))]
fn fn_param(param: u32) {}

#[instrument(fields(foo = "bar", empty))]
fn fn_empty_field() {}

#[instrument(fields(len = s.len()))]
fn fn_expr_field(s: &str) {}

#[instrument(fields(s.len = s.len(), s.is_empty = s.is_empty()))]
fn fn_two_expr_fields(s: &str) {
    let _ = s;
}

#[instrument(fields(%s, s.len = s.len()))]
fn fn_clashy_expr_field(s: &str) {
    let _ = s;
}

#[instrument(fields(s = "s"))]
fn fn_clashy_expr_field2(s: &str) {
    let _ = s;
}

#[instrument(fields(s = &s))]
fn fn_string(s: String) {
    let _ = s;
}

#[instrument(fields(keywords.impl.type.fn = _arg), skip(_arg))]
fn fn_keyword_ident_in_field(_arg: &str) {}

#[derive(Debug)]
struct HasField {
    my_field: &'static str,
}

impl HasField {
    #[instrument(fields(my_field = self.my_field), skip(self))]
    fn self_expr_field(&self) {}
}

#[test]
fn fields() {
    let span = expect::span().with_fields(
        expect::field("foo")
            .with_value(&"bar")
            .and(expect::field("dsa").with_value(&true))
            .and(expect::field("num").with_value(&1))
            .only(),
    );
    run_test(span, || {
        fn_no_param();
    });
}

#[test]
fn expr_field() {
    let span = expect::span().with_fields(
        expect::field("s")
            .with_value(&"hello world")
            .and(expect::field("len").with_value(&"hello world".len()))
            .only(),
    );
    run_test(span, || {
        fn_expr_field("hello world");
    });
}

#[test]
fn two_expr_fields() {
    let span = expect::span().with_fields(
        expect::field("s")
            .with_value(&"hello world")
            .and(expect::field("s.len").with_value(&"hello world".len()))
            .and(expect::field("s.is_empty").with_value(&false))
            .only(),
    );
    run_test(span, || {
        fn_two_expr_fields("hello world");
    });
}

#[test]
fn clashy_expr_field() {
    let span = expect::span().with_fields(
        // Overriding the `s` field should record `s` as a `Display` value,
        // rather than as a `Debug` value.
        expect::field("s")
            .with_value(&tracing::field::display("hello world"))
            .and(expect::field("s.len").with_value(&"hello world".len()))
            .only(),
    );
    run_test(span, || {
        fn_clashy_expr_field("hello world");
    });

    let span = expect::span().with_fields(expect::field("s").with_value(&"s").only());
    run_test(span, || {
        fn_clashy_expr_field2("hello world");
    });
}

#[test]
fn self_expr_field() {
    let span =
        expect::span().with_fields(expect::field("my_field").with_value(&"hello world").only());
    run_test(span, || {
        let has_field = HasField {
            my_field: "hello world",
        };
        has_field.self_expr_field();
    });
}

#[test]
fn parameters_with_fields() {
    let span = expect::span().with_fields(
        expect::field("foo")
            .with_value(&"bar")
            .and(expect::field("param").with_value(&1u32))
            .only(),
    );
    run_test(span, || {
        fn_param(1);
    });
}

#[test]
fn empty_field() {
    let span = expect::span().with_fields(expect::field("foo").with_value(&"bar").only());
    run_test(span, || {
        fn_empty_field();
    });
}

#[test]
fn string_field() {
    let span = expect::span().with_fields(expect::field("s").with_value(&"hello world").only());
    run_test(span, || {
        fn_string(String::from("hello world"));
    });
}

#[test]
fn keyword_ident_in_field_name() {
    let span = expect::span().with_fields(
        expect::field("keywords.impl.type.fn")
            .with_value(&"test")
            .only(),
    );
    run_test(span, || fn_keyword_ident_in_field("test"));
}

fn run_test<F: FnOnce() -> T, T>(span: NewSpan, fun: F) {
    let (subscriber, handle) = subscriber::mock()
        .new_span(span)
        .enter(expect::span())
        .exit(expect::span())
        .only()
        .run_with_handle();

    with_default(subscriber, fun);
    handle.assert_finished();
}
