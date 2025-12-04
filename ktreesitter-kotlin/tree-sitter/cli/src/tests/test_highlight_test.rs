use tree_sitter::Parser;
use tree_sitter_highlight::{Highlight, Highlighter};

use super::helpers::fixtures::{get_highlight_config, get_language, test_loader};
use crate::{
    query_testing::{parse_position_comments, Assertion, Utf8Point},
    test_highlight::get_highlight_positions,
};

#[test]
fn test_highlight_test_with_basic_test() {
    let language = get_language("javascript");
    let config = get_highlight_config(
        "javascript",
        Some("injections.scm"),
        &[
            "function".to_string(),
            "variable".to_string(),
            "keyword".to_string(),
        ],
    );
    let source = [
        "// hi",
        "var abc = function(d) {",
        "  // ^ function",
        "  //       ^^^ keyword",
        "  return d + e;",
        "  //     ^ variable",
        "  //       ^ !variable",
        "};",
        "var y̆y̆y̆y̆ = function() {}",
        "  // ^ function",
        "  //       ^ keyword",
    ]
    .join("\n");

    let assertions =
        parse_position_comments(&mut Parser::new(), &language, source.as_bytes()).unwrap();
    assert_eq!(
        assertions,
        &[
            Assertion::new(1, 5, 1, false, String::from("function")),
            Assertion::new(1, 11, 3, false, String::from("keyword")),
            Assertion::new(4, 9, 1, false, String::from("variable")),
            Assertion::new(4, 11, 1, true, String::from("variable")),
            Assertion::new(8, 5, 1, false, String::from("function")),
            Assertion::new(8, 11, 1, false, String::from("keyword")),
        ]
    );

    let mut highlighter = Highlighter::new();
    let highlight_positions =
        get_highlight_positions(test_loader(), &mut highlighter, &config, source.as_bytes())
            .unwrap();
    assert_eq!(
        highlight_positions,
        &[
            (Utf8Point::new(1, 0), Utf8Point::new(1, 3), Highlight(2)), // "var"
            (Utf8Point::new(1, 4), Utf8Point::new(1, 7), Highlight(0)), // "abc"
            (Utf8Point::new(1, 10), Utf8Point::new(1, 18), Highlight(2)), // "function"
            (Utf8Point::new(1, 19), Utf8Point::new(1, 20), Highlight(1)), // "d"
            (Utf8Point::new(4, 2), Utf8Point::new(4, 8), Highlight(2)), // "return"
            (Utf8Point::new(4, 9), Utf8Point::new(4, 10), Highlight(1)), // "d"
            (Utf8Point::new(4, 13), Utf8Point::new(4, 14), Highlight(1)), // "e"
            (Utf8Point::new(8, 0), Utf8Point::new(8, 3), Highlight(2)), // "var"
            (Utf8Point::new(8, 4), Utf8Point::new(8, 8), Highlight(0)), // "y̆y̆y̆y̆"
            (Utf8Point::new(8, 11), Utf8Point::new(8, 19), Highlight(2)), // "function"
        ]
    );
}
