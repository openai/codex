use indoc::formatdoc;

#[test]
fn carriage_return() {
    // Every line in the string ends with \r\n
    let indoc = formatdoc! {"
        {}

            \\{}
        {}",
        'a', 'b', 'c'
    };
    let expected = "a\n\n    \\b\nc";
    assert_eq!(indoc, expected);
}

#[test]
fn empty_string() {
    let indoc = formatdoc! {""};
    let expected = "";
    assert_eq!(indoc, expected);
}

#[test]
fn joined_first_line() {
    let indoc = formatdoc! {"\
        {}", 'a'
    };
    let expected = "a";
    assert_eq!(indoc, expected);
}

#[test]
fn joined_lines() {
    let indoc = formatdoc! {"
        {}\
        {}
        {}\
          {}
        {}",
        'a', 'b', 'c', 'd', 'e'
    };
    let expected = "ab\ncd\ne";
    assert_eq!(indoc, expected);
}

#[test]
fn no_leading_newline() {
    let indoc = formatdoc! {"{}
                             {}
                             {}", 'a', 'b', 'c'};
    let expected = "a\nb\nc";
    assert_eq!(indoc, expected);
}

#[test]
fn one_line() {
    let indoc = formatdoc! {"a"};
    let expected = "a";
    assert_eq!(indoc, expected);
}

#[test]
fn raw_string() {
    let indoc = formatdoc! {r#"
        {:?}"

            \\{}
        {}"#,
        "a", 'b', 'c'
    };
    let expected = "\"a\"\"\n\n    \\\\b\nc";
    assert_eq!(indoc, expected);
}

#[test]
fn string() {
    let indoc = formatdoc! {"
        {}

            \\{}
        {}",
        'a', 'b', 'c'
    };
    let expected = "a\n\n    \\b\nc";
    assert_eq!(indoc, expected);
}

#[test]
fn string_trailing_newline() {
    let indoc = formatdoc! {"
        {}

            \\{}
        {}
    ",
        'a', 'b', 'c'
    };
    let expected = "a\n\n    \\b\nc\n";
    assert_eq!(indoc, expected);
}

#[test]
fn trailing_whitespace() {
    let indoc = formatdoc! {"
        {} {below}
          
        {} {below}
        
        {} {below}
      
        end",
        2, 0, -2, below = "below"
    };
    let expected = "2 below\n  \n0 below\n\n-2 below\n\nend";
    assert_eq!(indoc, expected);
}
