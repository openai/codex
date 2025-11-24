//! This example shows an example of how to parse an escaped string. The
//! rules for the string are similar to JSON and rust. A string is:
//!
//! - Enclosed by double quotes
//! - Can contain any raw unescaped code point besides \ and "
//! - Matches the following escape sequences: \b, \f, \n, \r, \t, \", \\, \/
//! - Matches code points like Rust: \u{XXXX}, where XXXX can be up to 6
//!   hex characters
//! - an escape followed by whitespace consumes all whitespace between the
//!   escape and the next non-whitespace character

#![cfg(feature = "alloc")]

mod parser;

use winnow::prelude::*;

fn main() -> Result<(), lexopt::Error> {
    let args = Args::parse()?;

    let data = args.input.as_deref().unwrap_or("\"abc\"");
    let result = parser::parse_string::<()>.parse(data);
    match result {
        Ok(data) => println!("{data}"),
        Err(err) => println!("{err:?}"),
    }

    Ok(())
}

#[derive(Default)]
struct Args {
    input: Option<String>,
}

impl Args {
    fn parse() -> Result<Self, lexopt::Error> {
        use lexopt::prelude::*;

        let mut res = Args::default();

        let mut args = lexopt::Parser::from_env();
        while let Some(arg) = args.next()? {
            match arg {
                Value(input) => {
                    res.input = Some(input.string()?);
                }
                _ => return Err(arg.unexpected()),
            }
        }
        Ok(res)
    }
}

#[test]
fn simple() {
    let data = "\"abc\"";
    let result = parser::parse_string::<()>.parse(data);
    assert_eq!(result, Ok(String::from("abc")));
}

#[test]
fn escaped() {
    let data = "\"tab:\\tafter tab, newline:\\nnew line, quote: \\\", emoji: \\u{1F602}, newline:\\nescaped whitespace: \\    abc\"";
    let result = parser::parse_string::<()>.parse(data);
    assert_eq!(
        result,
        Ok(String::from("tab:\tafter tab, newline:\nnew line, quote: \", emoji: 😂, newline:\nescaped whitespace: abc"))
    );
}
