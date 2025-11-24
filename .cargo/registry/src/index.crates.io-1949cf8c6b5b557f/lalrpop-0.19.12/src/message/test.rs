use ascii_canvas::AsciiCanvas;
use crate::grammar::parse_tree::Span;
use crate::message::builder::MessageBuilder;
use crate::test_util::expect_debug;
use crate::tls::Tls;

use super::*;

fn install_tls() -> Tls {
    Tls::test_string(
        r#"foo
bar
baz
"#,
    )
}

#[test]
fn hello_world() {
    let _tls = install_tls();
    let msg = MessageBuilder::new(Span(0, 2))
        .heading()
        .text("Hello, world!")
        .end()
        .body()
        .begin_wrap()
        .text(
            "This is a very, very, very, very long sentence. \
             OK, not THAT long!",
        )
        .end()
        .indented_by(4)
        .end()
        .end();
    let min_width = msg.min_width();
    let mut canvas = AsciiCanvas::new(0, min_width);
    msg.emit(&mut canvas);
    expect_debug(
        &canvas.to_strings(),
        r#"
[
    "tmp.txt:1:1: 1:2: Hello, world!",
    "",
    "      This is a very, very,",
    "      very, very long sentence.",
    "      OK, not THAT long!"
]
"#
        .trim(),
    );
}

/// Test a case where the body in the message is longer than the
/// header (which used to mess up the `min_width` computation).
#[test]
fn long_body() {
    let _tls = install_tls();
    let msg = MessageBuilder::new(Span(0, 2))
        .heading()
        .text("Hello, world!")
        .end()
        .body()
        .text(
            "This is a very, very, very, very long sentence. \
             OK, not THAT long!",
        )
        .end()
        .end();
    let min_width = msg.min_width();
    let mut canvas = AsciiCanvas::new(0, min_width);
    msg.emit(&mut canvas);
    expect_debug(
        &canvas.to_strings(),
        r#"
[
    "tmp.txt:1:1: 1:2: Hello, world!",
    "",
    "  This is a very, very, very, very long sentence. OK, not THAT long!"
]
"#
        .trim(),
    );
}
