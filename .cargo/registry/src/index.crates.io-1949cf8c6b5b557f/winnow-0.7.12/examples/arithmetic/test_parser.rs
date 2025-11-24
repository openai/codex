use snapbox::assert_data_eq;
use snapbox::prelude::*;
use snapbox::str;
use winnow::prelude::*;

use crate::parser::*;

#[test]
fn factor_test() {
    let input = "3";
    let expected = str![[r#"
Ok(
    (
        "",
        3,
    ),
)

"#]];
    assert_data_eq!(factor.parse_peek(input).to_debug(), expected);

    let input = " 12";
    let expected = str![[r#"
Ok(
    (
        "",
        12,
    ),
)

"#]];
    assert_data_eq!(factor.parse_peek(input).to_debug(), expected);

    let input = "537 ";
    let expected = str![[r#"
Ok(
    (
        "",
        537,
    ),
)

"#]];
    assert_data_eq!(factor.parse_peek(input).to_debug(), expected);

    let input = "  24     ";
    let expected = str![[r#"
Ok(
    (
        "",
        24,
    ),
)

"#]];
    assert_data_eq!(factor.parse_peek(input).to_debug(), expected);
}

#[test]
fn term_test() {
    let input = " 12 *2 /  3";
    let expected = str![[r#"
Ok(
    (
        "",
        8,
    ),
)

"#]];
    assert_data_eq!(term.parse_peek(input).to_debug(), expected);

    let input = " 12 *2 /  3";
    let expected = str![[r#"
Ok(
    (
        "",
        8,
    ),
)

"#]];
    assert_data_eq!(term.parse_peek(input).to_debug(), expected);

    let input = " 2* 3  *2 *2 /  3";
    let expected = str![[r#"
Ok(
    (
        "",
        8,
    ),
)

"#]];
    assert_data_eq!(term.parse_peek(input).to_debug(), expected);

    let input = " 48 /  3/2";
    let expected = str![[r#"
Ok(
    (
        "",
        8,
    ),
)

"#]];
    assert_data_eq!(term.parse_peek(input).to_debug(), expected);
}

#[test]
fn expr_test() {
    let input = " 1 +  2 ";
    let expected = str![[r#"
Ok(
    (
        "",
        3,
    ),
)

"#]];
    assert_data_eq!(expr.parse_peek(input).to_debug(), expected);

    let input = " 12 + 6 - 4+  3";
    let expected = str![[r#"
Ok(
    (
        "",
        17,
    ),
)

"#]];
    assert_data_eq!(expr.parse_peek(input).to_debug(), expected);

    let input = " 1 + 2*3 + 4";
    let expected = str![[r#"
Ok(
    (
        "",
        11,
    ),
)

"#]];
    assert_data_eq!(expr.parse_peek(input).to_debug(), expected);
}

#[test]
fn parens_test() {
    let input = " (  2 )";
    let expected = str![[r#"
Ok(
    (
        "",
        2,
    ),
)

"#]];
    assert_data_eq!(expr.parse_peek(input).to_debug(), expected);

    let input = " 2* (  3 + 4 ) ";
    let expected = str![[r#"
Ok(
    (
        "",
        14,
    ),
)

"#]];
    assert_data_eq!(expr.parse_peek(input).to_debug(), expected);

    let input = "  2*2 / ( 5 - 1) + 3";
    let expected = str![[r#"
Ok(
    (
        "",
        4,
    ),
)

"#]];
    assert_data_eq!(expr.parse_peek(input).to_debug(), expected);
}
