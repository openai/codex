use diff;
use crate::grammar::parse_tree as pt;
use crate::grammar::repr as r;
use crate::normalize::NormError;
use regex::Regex;
use std::fmt::{Debug, Error, Formatter};

thread_local! {
    static SPAN: Regex =
        Regex::new(r"Span\([0-9 ,\n]*\)").unwrap()
}

struct ExpectedDebug<'a>(&'a str);

impl<'a> Debug for ExpectedDebug<'a> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        // Ignore trailing commas in multiline Debug representation.
        // Needed to work around rust-lang/rust#59076.
        let s = self.0.replace(",\n", "\n");
        write!(fmt, "{}", s)
    }
}

pub fn expect_debug<D: Debug>(actual: D, expected: &str) {
    compare(
        ExpectedDebug(&format!("{:#?}", actual)),
        ExpectedDebug(expected),
    )
}

pub fn compare<D: Debug, E: Debug>(actual: D, expected: E) {
    let actual_s = format!("{:?}", actual);
    let expected_s = format!("{:?}", expected);

    if normalize(&actual_s) != normalize(&expected_s) {
        let actual_s = format!("{:#?}", actual);
        let expected_s = format!("{:#?}", expected);

        for diff in diff::lines(&normalize(&actual_s), &normalize(&expected_s)) {
            match diff {
                diff::Result::Right(r) => println!("- {}", r),
                diff::Result::Left(l) => println!("+ {}", l),
                diff::Result::Both(l, _) => println!("  {}", l),
            }
        }

        assert!(false);
    }

    /// Ignore differences in `Span` values, by replacing them all with fixed
    /// dummy text.
    fn normalize<'t>(with_spans: &'t str) -> std::borrow::Cow<'t, str> {
        SPAN.with(|span| {
            span.replace_all(with_spans, "Span(..)")
        })
    }
}

pub fn normalized_grammar(s: &str) -> r::Grammar {
    crate::normalize::normalize_without_validating(crate::parser::parse_grammar(s).unwrap()).unwrap()
}

pub fn check_norm_err(expected_err: &str, span: &str, err: NormError) {
    let expected_err = Regex::new(expected_err).unwrap();
    let start_index = span.find('~').unwrap();
    let end_index = span.rfind('~').unwrap() + 1;
    assert!(
        expected_err.is_match(&err.message),
        "unexpected error text `{}`, which did not match regular expression `{}`",
        err.message,
        expected_err
    );
    assert!(start_index <= end_index);
    assert_eq!(err.span, pt::Span(start_index, end_index));
}
