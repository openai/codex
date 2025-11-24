#![allow(
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes,
    clippy::needless_raw_string_hashes,
    clippy::trivially_copy_pass_by_ref,
    clippy::uninlined_format_args
)]

use core::fmt::{self, Display};
use thiserror::Error;

fn assert<T: Display>(expected: &str, value: T) {
    assert_eq!(expected, value.to_string());
}

#[test]
fn test_braced() {
    #[derive(Error, Debug)]
    #[error("braced error: {msg}")]
    struct Error {
        msg: String,
    }

    let msg = "T".to_owned();
    assert("braced error: T", Error { msg });
}

#[test]
fn test_braced_unused() {
    #[derive(Error, Debug)]
    #[error("braced error")]
    struct Error {
        extra: usize,
    }

    assert("braced error", Error { extra: 0 });
}

#[test]
fn test_tuple() {
    #[derive(Error, Debug)]
    #[error("tuple error: {0}")]
    struct Error(usize);

    assert("tuple error: 0", Error(0));
}

#[test]
fn test_unit() {
    #[derive(Error, Debug)]
    #[error("unit error")]
    struct Error;

    assert("unit error", Error);
}

#[test]
fn test_enum() {
    #[derive(Error, Debug)]
    enum Error {
        #[error("braced error: {id}")]
        Braced { id: usize },
        #[error("tuple error: {0}")]
        Tuple(usize),
        #[error("unit error")]
        Unit,
    }

    assert("braced error: 0", Error::Braced { id: 0 });
    assert("tuple error: 0", Error::Tuple(0));
    assert("unit error", Error::Unit);
}

#[test]
fn test_constants() {
    #[derive(Error, Debug)]
    #[error("{MSG}: {id:?} (code {CODE:?})")]
    struct Error {
        id: &'static str,
    }

    const MSG: &str = "failed to do";
    const CODE: usize = 9;

    assert("failed to do: \"\" (code 9)", Error { id: "" });
}

#[test]
fn test_inherit() {
    #[derive(Error, Debug)]
    #[error("{0}")]
    enum Error {
        Some(&'static str),
        #[error("other error")]
        Other(&'static str),
    }

    assert("some error", Error::Some("some error"));
    assert("other error", Error::Other("..."));
}

#[test]
fn test_brace_escape() {
    #[derive(Error, Debug)]
    #[error("fn main() {{}}")]
    struct Error;

    assert("fn main() {}", Error);
}

#[test]
fn test_expr() {
    #[derive(Error, Debug)]
    #[error("1 + 1 = {}", 1 + 1)]
    struct Error;
    assert("1 + 1 = 2", Error);
}

#[test]
fn test_nested() {
    #[derive(Error, Debug)]
    #[error("!bool = {}", not(.0))]
    struct Error(bool);

    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn not(bool: &bool) -> bool {
        !*bool
    }

    assert("!bool = false", Error(true));
}

#[test]
fn test_match() {
    #[derive(Error, Debug)]
    #[error("{intro}: {0}", intro = match .1 {
        Some(n) => format!("error occurred with {}", n),
        None => "there was an empty error".to_owned(),
    })]
    struct Error(String, Option<usize>);

    assert(
        "error occurred with 1: ...",
        Error("...".to_owned(), Some(1)),
    );
    assert(
        "there was an empty error: ...",
        Error("...".to_owned(), None),
    );
}

#[test]
fn test_nested_display() {
    // Same behavior as the one in `test_match`, but without String allocations.
    #[derive(Error, Debug)]
    #[error("{}", {
        struct Msg<'a>(&'a String, &'a Option<usize>);
        impl<'a> Display for Msg<'a> {
            fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                match self.1 {
                    Some(n) => write!(formatter, "error occurred with {}", n),
                    None => write!(formatter, "there was an empty error"),
                }?;
                write!(formatter, ": {}", self.0)
            }
        }
        Msg(.0, .1)
    })]
    struct Error(String, Option<usize>);

    assert(
        "error occurred with 1: ...",
        Error("...".to_owned(), Some(1)),
    );
    assert(
        "there was an empty error: ...",
        Error("...".to_owned(), None),
    );
}

#[test]
fn test_void() {
    #[allow(clippy::empty_enum)]
    #[derive(Error, Debug)]
    #[error("...")]
    pub enum Error {}

    let _: Error;
}

#[test]
fn test_mixed() {
    #[derive(Error, Debug)]
    #[error("a={a} :: b={} :: c={c} :: d={d}", 1, c = 2, d = 3)]
    struct Error {
        a: usize,
        d: usize,
    }

    assert("a=0 :: b=1 :: c=2 :: d=3", Error { a: 0, d: 0 });
}

#[test]
fn test_ints() {
    #[derive(Error, Debug)]
    enum Error {
        #[error("error {0}")]
        Tuple(usize, usize),
        #[error("error {0}", '?')]
        Struct { v: usize },
    }

    assert("error 9", Error::Tuple(9, 0));
    assert("error ?", Error::Struct { v: 0 });
}

#[test]
fn test_trailing_comma() {
    #[derive(Error, Debug)]
    #[error(
        "error {0}",
    )]
    #[rustfmt::skip]
    struct Error(char);

    assert("error ?", Error('?'));
}

#[test]
fn test_field() {
    #[derive(Debug)]
    struct Inner {
        data: usize,
    }

    #[derive(Error, Debug)]
    #[error("{}", .0.data)]
    struct Error(Inner);

    assert("0", Error(Inner { data: 0 }));
}

#[test]
fn test_nested_tuple_field() {
    #[derive(Debug)]
    struct Inner(usize);

    #[derive(Error, Debug)]
    #[error("{}", .0.0)]
    struct Error(Inner);

    assert("0", Error(Inner(0)));
}

#[test]
fn test_pointer() {
    #[derive(Error, Debug)]
    #[error("{field:p}")]
    pub struct Struct {
        field: Box<i32>,
    }

    let s = Struct {
        field: Box::new(-1),
    };
    assert_eq!(s.to_string(), format!("{:p}", s.field));
}

#[test]
fn test_macro_rules_variant_from_call_site() {
    // Regression test for https://github.com/dtolnay/thiserror/issues/86

    macro_rules! decl_error {
        ($variant:ident($value:ident)) => {
            #[derive(Error, Debug)]
            pub enum Error0 {
                #[error("{0:?}")]
                $variant($value),
            }

            #[derive(Error, Debug)]
            #[error("{0:?}")]
            pub enum Error1 {
                $variant($value),
            }
        };
    }

    decl_error!(Repro(u8));

    assert("0", Error0::Repro(0));
    assert("0", Error1::Repro(0));
}

#[test]
fn test_macro_rules_message_from_call_site() {
    // Regression test for https://github.com/dtolnay/thiserror/issues/398

    macro_rules! decl_error {
        ($($errors:tt)*) => {
            #[derive(Error, Debug)]
            pub enum Error {
                $($errors)*
            }
        };
    }

    decl_error! {
        #[error("{0}")]
        Unnamed(u8),
        #[error("{x}")]
        Named { x: u8 },
    }

    assert("0", Error::Unnamed(0));
    assert("0", Error::Named { x: 0 });
}

#[test]
fn test_raw() {
    #[derive(Error, Debug)]
    #[error("braced raw error: {fn}")]
    struct Error {
        r#fn: &'static str,
    }

    assert("braced raw error: T", Error { r#fn: "T" });
}

#[test]
fn test_raw_enum() {
    #[derive(Error, Debug)]
    enum Error {
        #[error("braced raw error: {fn}")]
        Braced { r#fn: &'static str },
    }

    assert("braced raw error: T", Error::Braced { r#fn: "T" });
}

#[test]
fn test_keyword() {
    #[derive(Error, Debug)]
    #[error("error: {type}", type = 1)]
    struct Error;

    assert("error: 1", Error);
}

#[test]
fn test_self() {
    #[derive(Error, Debug)]
    #[error("error: {self:?}")]
    struct Error;

    assert("error: Error", Error);
}

#[test]
fn test_str_special_chars() {
    #[derive(Error, Debug)]
    pub enum Error {
        #[error("brace left {{")]
        BraceLeft,
        #[error("brace left 2 \x7B\x7B")]
        BraceLeft2,
        #[error("brace left 3 \u{7B}\u{7B}")]
        BraceLeft3,
        #[error("brace right }}")]
        BraceRight,
        #[error("brace right 2 \x7D\x7D")]
        BraceRight2,
        #[error("brace right 3 \u{7D}\u{7D}")]
        BraceRight3,
        #[error(
            "new_\
line"
        )]
        NewLine,
        #[error("escape24 \u{78}")]
        Escape24,
    }

    assert("brace left {", Error::BraceLeft);
    assert("brace left 2 {", Error::BraceLeft2);
    assert("brace left 3 {", Error::BraceLeft3);
    assert("brace right }", Error::BraceRight);
    assert("brace right 2 }", Error::BraceRight2);
    assert("brace right 3 }", Error::BraceRight3);
    assert("new_line", Error::NewLine);
    assert("escape24 x", Error::Escape24);
}

#[test]
fn test_raw_str() {
    #[derive(Error, Debug)]
    pub enum Error {
        #[error(r#"raw brace left {{"#)]
        BraceLeft,
        #[error(r#"raw brace left 2 \x7B"#)]
        BraceLeft2,
        #[error(r#"raw brace right }}"#)]
        BraceRight,
        #[error(r#"raw brace right 2 \x7D"#)]
        BraceRight2,
    }

    assert(r#"raw brace left {"#, Error::BraceLeft);
    assert(r#"raw brace left 2 \x7B"#, Error::BraceLeft2);
    assert(r#"raw brace right }"#, Error::BraceRight);
    assert(r#"raw brace right 2 \x7D"#, Error::BraceRight2);
}

mod util {
    use core::fmt::{self, Octal};

    pub fn octal<T: Octal>(value: &T, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "0o{:o}", value)
    }
}

#[test]
fn test_fmt_path() {
    fn unit(formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("unit=")
    }

    fn pair(k: &i32, v: &i32, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "pair={k}:{v}")
    }

    #[derive(Error, Debug)]
    pub enum Error {
        #[error(fmt = unit)]
        Unit,
        #[error(fmt = pair)]
        Tuple(i32, i32),
        #[error(fmt = pair)]
        Entry { k: i32, v: i32 },
        #[error(fmt = crate::util::octal)]
        I16(i16),
        #[error(fmt = crate::util::octal::<i32>)]
        I32 { n: i32 },
        #[error(fmt = core::fmt::Octal::fmt)]
        I64(i64),
        #[error("...{0}")]
        Other(bool),
    }

    assert("unit=", Error::Unit);
    assert("pair=10:0", Error::Tuple(10, 0));
    assert("pair=10:0", Error::Entry { k: 10, v: 0 });
    assert("0o777", Error::I16(0o777));
    assert("0o777", Error::I32 { n: 0o777 });
    assert("777", Error::I64(0o777));
    assert("...false", Error::Other(false));
}

#[test]
fn test_fmt_path_inherited() {
    #[derive(Error, Debug)]
    #[error(fmt = crate::util::octal)]
    pub enum Error {
        I16(i16),
        I32 {
            n: i32,
        },
        #[error(fmt = core::fmt::Octal::fmt)]
        I64(i64),
        #[error("...{0}")]
        Other(bool),
    }

    assert("0o777", Error::I16(0o777));
    assert("0o777", Error::I32 { n: 0o777 });
    assert("777", Error::I64(0o777));
    assert("...false", Error::Other(false));
}
