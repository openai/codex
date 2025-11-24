#![allow(clippy::if_same_then_else, clippy::let_underscore_untyped)]

use indoc::writedoc;
use std::fmt::Write as _;

#[test]
fn test_write_to_string() {
    let mut s = String::new();
    writedoc!(
        s,
        "
        one
        two
        ",
    )
    .unwrap();

    let expected = "one\ntwo\n";
    assert_eq!(s, expected);
}

#[test]
fn test_format_args() {
    let mut s = String::new();
    writedoc!(
        s,
        "
        {}
        {}
        ",
        0,
        0,
    )
    .unwrap();

    let expected = "0\n0\n";
    assert_eq!(s, expected);
}

#[test]
fn test_angle_bracket_parsing() {
    const ZERO: usize = 0;

    struct Pair<A, B>(A, B);
    impl<A, B> Pair<A, B> {
        const ONE: usize = 1;
    }

    let mut s = String::new();
    let _ = writedoc! {
        if ZERO < Pair::<fn() -> (), ()>::ONE { &mut s } else { &mut s },
        "writedoc",
    };

    let expected = "writedoc";
    assert_eq!(s, expected);
}
