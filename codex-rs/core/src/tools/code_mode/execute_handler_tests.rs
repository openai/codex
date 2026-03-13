use super::parse_freeform_args;
use pretty_assertions::assert_eq;

#[test]
fn parse_freeform_args_without_pragma() {
    let args = parse_freeform_args("output_text('ok');").expect("parse args");
    assert_eq!(args.code, "output_text('ok');");
    assert_eq!(args.yield_time_ms, None);
    assert_eq!(args.max_output_tokens, None);
}

#[test]
fn parse_freeform_args_with_pragma() {
    let input = concat!(
        "// @exec: {\"yield_time_ms\": 15000, \"max_output_tokens\": 2000}\n",
        "output_text('ok');",
    );
    let args = parse_freeform_args(input).expect("parse args");
    assert_eq!(args.code, "output_text('ok');");
    assert_eq!(args.yield_time_ms, Some(15_000));
    assert_eq!(args.max_output_tokens, Some(2_000));
}

#[test]
fn parse_freeform_args_rejects_unknown_key() {
    let err = parse_freeform_args("// @exec: {\"nope\": 1}\noutput_text('ok');")
        .expect_err("expected error");
    assert_eq!(
        err.to_string(),
        "exec pragma only supports `yield_time_ms` and `max_output_tokens`; got `nope`"
    );
}

#[test]
fn parse_freeform_args_rejects_missing_source() {
    let err = parse_freeform_args("// @exec: {\"yield_time_ms\": 10}").expect_err("expected error");
    assert_eq!(
        err.to_string(),
        "exec pragma must be followed by JavaScript source on subsequent lines"
    );
}
