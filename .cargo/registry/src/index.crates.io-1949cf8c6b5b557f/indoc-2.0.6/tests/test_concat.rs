use indoc::concatdoc;

macro_rules! env {
    ($var:literal) => {
        "test"
    };
}

static HELP: &str = concatdoc! {"
    Usage: ", env!("CARGO_BIN_NAME"), " [options]

    Options:
        -h, --help
"};

#[test]
fn test_help() {
    let expected = "Usage: test [options]\n\nOptions:\n    -h, --help\n";
    assert_eq!(HELP, expected);
}
