use std::fmt::{Debug, Error, Formatter};

struct ExpectedDebug<'a>(&'a str);

impl<'a> Debug for ExpectedDebug<'a> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), Error> {
        write!(fmt, "{}", self.0)
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

    if actual_s != expected_s {
        let actual_s = format!("{:#?}", actual);
        let expected_s = format!("{:#?}", expected);

        for diff in diff::lines(&actual_s, &expected_s) {
            match diff {
                diff::Result::Right(r) => println!("- {}", r),
                diff::Result::Left(l) => println!("+ {}", l),
                diff::Result::Both(l, _) => println!("  {}", l),
            }
        }

        assert!(false);
    }
}
