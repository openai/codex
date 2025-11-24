use pulldown_cmark::{Options, Parser};

fn parse(md: &str) {
    let parser = Parser::new(md);

    for _ in parser {}
}

fn parse_all_options(md: &str) {
    let parser = Parser::new_ext(md, Options::all());

    for _ in parser {}
}

#[test]
fn test_lists_inside_code_spans() {
    parse(
        r"- `
x
**
  *
  `",
    );
}

#[test]
fn test_fuzzer_input_1() {
    parse(">\n >>><N\n");
}

#[test]
fn test_fuzzer_input_2() {
    parse(" \u{b}\\\r- ");
}

#[test]
fn test_fuzzer_input_3() {
    parse_all_options("\n # #\r\u{1c} ");
}

#[test]
fn test_fuzzer_input_4() {
    parse_all_options("\u{0}{\tϐ}\n-");
}

#[test]
fn test_fuzzer_input_5() {
    parse_all_options(" \u{c}{}\n-\n");
}

#[test]
fn test_fuzzer_input_6() {
    parse("*\t[][\n\t<p]>\n\t[]");
}

#[test]
fn test_fuzzer_input_7() {
    parse_all_options("[][{]}\n-");
}

#[test]
fn test_fuzzer_input_8() {
    parse_all_options("a\n \u{c}{}\n-");
}

#[test]
fn test_fuzzer_input_9() {
    parse_all_options("a\n \u{c}{}\\\n-");
}

#[test]
fn test_fuzzer_input_10() {
    parse_all_options("[[    \t\n   \u{c}\u{c}\u{c}\u{c}\u{c}    {}\n-\r\u{e}\u{0}\u{0}{# }\n\u{b}\u{b}\u{b}\u{b}\u{b}\u{b}\u{b}\u{b}\u{b}\u{b}\u{0}\u{0}");
}

#[test]
fn test_fuzzer_input_11() {
    parse_all_options(
        "[[\u{c}\u{c}   \t\n   \u{c}\u{c}\u{c}\u{c}\u{c}\u{c}\u{c}\u{c}\u{c}       {}\n-\r\u{e}",
    );
}

#[test]
fn test_fuzzer_input_12() {
    parse_all_options("\u{c}-\n\u{c}\n-");
}

#[test]
fn test_fuzzer_input_13() {
    // Does not fail with Options::all():
    Parser::new_ext(
        ".\r> ^](\r\u{c}\r\0\0\r.\r[^\0\0\\\0\0\0^^^^^]",
        Options::ENABLE_FOOTNOTES,
    );
}

#[test]
fn test_wrong_code_block() {
    parse(
        r##"```
 * ```
 "##,
    );
}

#[test]
fn test_unterminated_link() {
    parse("[](\\");
}

#[test]
fn test_unterminated_autolink() {
    parse("<a");
}

#[test]
fn test_infinite_loop() {
    parse("[<!W\n\\\n");
}

#[test]
fn test_html_tag() {
    parse("<script\u{feff}");
}

// all of test_bad_slice_* were found in https://github.com/raphlinus/pulldown-cmark/issues/521
#[test]
fn test_bad_slice_a() {
    parse("><a\n");
}

#[test]
fn test_bad_slice_b() {
    parse("><a a\n");
}

#[test]
fn test_bad_slice_unicode() {
    parse("><a a=\n毿>")
}
