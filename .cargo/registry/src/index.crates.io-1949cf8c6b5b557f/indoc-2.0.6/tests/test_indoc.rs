use indoc::indoc;

const HELP: &str = indoc! {"
    Usage: ./foo
"};

#[test]
fn test_global() {
    let expected = "Usage: ./foo\n";
    assert_eq!(HELP, expected);
}

#[test]
fn byte_string() {
    let indoc = indoc! {b"
        a

            \\b
        c"
    };
    let expected = b"a\n\n    \\b\nc";
    assert_eq!(indoc, expected);
}

#[test]
fn carriage_return() {
    // Every line in the string ends with \r\n
    let indoc = indoc! {"
        a

            \\b
        c"
    };
    let expected = "a\n\n    \\b\nc";
    assert_eq!(indoc, expected);
}

#[test]
fn trailing_comma() {
    let indoc = indoc! {
        "
        test
        ",
    };
    let expected = "test\n";
    assert_eq!(indoc, expected);
}

#[test]
fn empty_string() {
    let indoc = indoc! {""};
    let expected = "";
    assert_eq!(indoc, expected);
}

#[test]
fn joined_first_line() {
    let indoc = indoc! {"\
        a"
    };
    let expected = "a";
    assert_eq!(indoc, expected);
}

#[test]
fn joined_lines() {
    let indoc = indoc! {"
        a\
        b
        c\
          d
        e"
    };
    let expected = "ab\ncd\ne";
    assert_eq!(indoc, expected);
}

#[test]
fn no_leading_newline() {
    let indoc = indoc! {"a
                         b
                         c"};
    let expected = "a\nb\nc";
    assert_eq!(indoc, expected);
}

#[test]
fn one_line() {
    let indoc = indoc! {"a"};
    let expected = "a";
    assert_eq!(indoc, expected);
}

#[test]
fn raw_byte_string() {
    let indoc = indoc! {br#"
        "a"

            \\b
        c"#
    };
    let expected = b"\"a\"\n\n    \\\\b\nc";
    assert_eq!(indoc, expected);
}

#[test]
fn raw_string() {
    let indoc = indoc! {r#"
        "a"

            \\b
        c"#
    };
    let expected = "\"a\"\n\n    \\\\b\nc";
    assert_eq!(indoc, expected);
}

#[test]
fn string() {
    let indoc = indoc! {"
        a

            \\b
        c"
    };
    let expected = "a\n\n    \\b\nc";
    assert_eq!(indoc, expected);
}

#[test]
fn string_trailing_newline() {
    let indoc = indoc! {"
        a

            \\b
        c
    "};
    let expected = "a\n\n    \\b\nc\n";
    assert_eq!(indoc, expected);
}

#[test]
fn trailing_whitespace() {
    let indoc = indoc! {"
        2 below
          
        0 below
        
        -2 below
      
        end"
    };
    let expected = "2 below\n  \n0 below\n\n-2 below\n\nend";
    assert_eq!(indoc, expected);
}

#[test]
fn indoc_as_format_string() {
    let s = format!(indoc! {"{}"}, true);
    assert_eq!(s, "true");
}

#[test]
fn test_metavariable() {
    macro_rules! indoc_wrapper {
        ($e:expr) => {
            indoc!($e)
        };
    }

    let indoc = indoc_wrapper! {"
        macros, how do they work
    "};
    let expected = "macros, how do they work\n";
    assert_eq!(indoc, expected);
}
