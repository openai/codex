// Copyright 2018 Tomasz Miąsko
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE>
// or the MIT license <LICENSE-MIT>, at your option.
//
//! Process command line according to parsing rules of Unix shell as specified
//! in [Shell Command Language in POSIX.1-2008][posix-shell].
//!
//! [posix-shell]: http://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "std")]
extern crate core;

use core::fmt;
use core::mem;

#[cfg(not(feature = "std"))]
#[macro_use]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::borrow::Cow;
#[cfg(feature = "std")]
use std::borrow::Cow;

/// An error returned when shell parsing fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError;

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("missing closing quote")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

enum State {
    /// Within a delimiter.
    Delimiter,
    /// After backslash, but before starting word.
    Backslash,
    /// Within an unquoted word.
    Unquoted,
    /// After backslash in an unquoted word.
    UnquotedBackslash,
    /// Within a single quoted word.
    SingleQuoted,
    /// Within a double quoted word.
    DoubleQuoted,
    /// After backslash inside a double quoted word.
    DoubleQuotedBackslash,
    /// Inside a comment.
    Comment,
}

/// Splits command line into separate arguments, in much the same way Unix shell
/// would, but without many of expansion the shell would perform.
///
/// The split functionality is compatible with behaviour of Unix shell, but with
/// word expansions limited to quote removal, and without special token
/// recognition rules for operators.
///
/// The result is exactly the same as one obtained from Unix shell as long as
/// those unsupported features are not present in input: no operators, no
/// variable assignments, no tilde expansion, no parameter expansion, no command
/// substitution, no arithmetic expansion, no pathname expansion.
///
/// In case those unsupported shell features are present, the syntax that
/// introduce them is interpreted literally.
///
/// # Errors
///
/// When input contains unmatched quote, an error is returned.
///
/// # Compatibility with other implementations
///
/// It should be fully compatible with g_shell_parse_argv from GLib, except that
/// in GLib it is an error not to have any words after tokenization.
///
/// It is also very close to shlex.split available in Python standard library,
/// when used in POSIX mode with support for comments. Though, shlex
/// implementation diverges from POSIX, and from implementation contained herein
/// in three aspects. First, it doesn't support line continuations.
/// Second, inside double quotes, the backslash characters retains its special
/// meaning as an escape character only when followed by \\ or \", whereas POSIX
/// specifies that it should retain its special meaning when followed by: $, \`,
/// \", \\, or a newline. Third, it treats carriage return as one of delimiters.
///
/// # Examples
///
/// Building an executable using compiler obtained from CC environment variable
/// and compiler flags from both CFLAGS and CPPFLAGS. Similar to default build
/// rule for C used in GNU Make:
///
/// ```rust,no_run
/// use std::env::var;
/// use std::process::Command;
///
/// let cc = var("CC").unwrap_or_else(|_| "cc".to_owned());
///
/// let cflags_str = var("CFLAGS").unwrap_or_else(|_| String::new());
/// let cflags = shell_words::split(&cflags_str).expect("failed to parse CFLAGS");
///
/// let cppflags_str = var("CPPFLAGS").unwrap_or_else(|_| String::new());
/// let cppflags = shell_words::split(&cppflags_str).expect("failed to parse CPPFLAGS");
///
/// Command::new(cc)
///     .args(cflags)
///     .args(cppflags)
///     .args(&["-c", "a.c", "-o", "a.out"])
///     .spawn()
///     .expect("failed to start subprocess")
///     .wait()
///     .expect("failed to wait for subprocess");
/// ```
pub fn split(s: &str) -> Result<Vec<String>, ParseError> {
    use State::*;

    let mut words = Vec::new();
    let mut word = String::new();
    let mut chars = s.chars();
    let mut state = Delimiter;

    loop {
        let c = chars.next();
        state = match state {
            Delimiter => match c {
                None => break,
                Some('\'') => SingleQuoted,
                Some('\"') => DoubleQuoted,
                Some('\\') => Backslash,
                Some('\t') | Some(' ') | Some('\n') => Delimiter,
                Some('#') => Comment,
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            Backslash => match c {
                None => {
                    word.push('\\');
                    words.push(mem::replace(&mut word, String::new()));
                    break;
                }
                Some('\n') => Delimiter,
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            Unquoted => match c {
                None => {
                    words.push(mem::replace(&mut word, String::new()));
                    break;
                }
                Some('\'') => SingleQuoted,
                Some('\"') => DoubleQuoted,
                Some('\\') => UnquotedBackslash,
                Some('\t') | Some(' ') | Some('\n') => {
                    words.push(mem::replace(&mut word, String::new()));
                    Delimiter
                }
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            UnquotedBackslash => match c {
                None => {
                    word.push('\\');
                    words.push(mem::replace(&mut word, String::new()));
                    break;
                }
                Some('\n') => Unquoted,
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            SingleQuoted => match c {
                None => return Err(ParseError),
                Some('\'') => Unquoted,
                Some(c) => {
                    word.push(c);
                    SingleQuoted
                }
            },
            DoubleQuoted => match c {
                None => return Err(ParseError),
                Some('\"') => Unquoted,
                Some('\\') => DoubleQuotedBackslash,
                Some(c) => {
                    word.push(c);
                    DoubleQuoted
                }
            },
            DoubleQuotedBackslash => match c {
                None => return Err(ParseError),
                Some('\n') => DoubleQuoted,
                Some(c @ '$') | Some(c @ '`') | Some(c @ '"') | Some(c @ '\\') => {
                    word.push(c);
                    DoubleQuoted
                }
                Some(c) => {
                    word.push('\\');
                    word.push(c);
                    DoubleQuoted
                }
            },
            Comment => match c {
                None => break,
                Some('\n') => Delimiter,
                Some(_) => Comment,
            },
        }
    }

    Ok(words)
}

enum EscapeStyle {
    /// No escaping.
    None,
    /// Wrap in single quotes.
    SingleQuoted,
    /// Single quotes combined with backslash.
    Mixed,
}

/// Determines escaping style to use.
fn escape_style(s: &str) -> EscapeStyle {
    if s.is_empty() {
        return EscapeStyle::SingleQuoted;
    }

    let mut special = false;
    let mut newline = false;
    let mut single_quote = false;

    for c in s.chars() {
        match c {
            '\n' => {
                newline = true;
                special = true;
            }
            '\'' => {
                single_quote = true;
                special = true;
            }
            '|' | '&' | ';' | '<' | '>' | '(' | ')' | '$' | '`' | '\\' | '"' | ' ' | '\t' | '*'
            | '?' | '[' | '#' | '˜' | '=' | '%' => {
                special = true;
            }
            _ => continue,
        }
    }

    if !special {
        EscapeStyle::None
    } else if newline && !single_quote {
        EscapeStyle::SingleQuoted
    } else {
        EscapeStyle::Mixed
    }
}

/// Escapes special characters in a string, so that it will retain its literal
/// meaning when used as a part of command in Unix shell.
///
/// It tries to avoid introducing any unnecessary quotes or escape characters,
/// but specifics regarding quoting style are left unspecified.
pub fn quote(s: &str) -> Cow<str> {
    // We are going somewhat out of the way to provide
    // minimal amount of quoting in typical cases.
    match escape_style(s) {
        EscapeStyle::None => s.into(),
        EscapeStyle::SingleQuoted => format!("'{}'", s).into(),
        EscapeStyle::Mixed => {
            let mut quoted = String::new();
            quoted.push('\'');
            for c in s.chars() {
                if c == '\'' {
                    quoted.push_str("'\\''");
                } else {
                    quoted.push(c);
                }
            }
            quoted.push('\'');
            quoted.into()
        }
    }
}

/// Joins arguments into a single command line suitable for execution in Unix
/// shell.
///
/// Each argument is quoted using [`quote`] to preserve its literal meaning when
/// parsed by Unix shell.
///
/// Note: This function is essentially an inverse of [`split`].
///
/// # Examples
///
/// Logging executed commands in format that can be easily copied and pasted
/// into an actual shell:
///
/// ```rust,no_run
/// fn execute(args: &[&str]) {
///     use std::process::Command;
///     println!("Executing: {}", shell_words::join(args));
///     Command::new(&args[0])
///         .args(&args[1..])
///         .spawn()
///         .expect("failed to start subprocess")
///         .wait()
///         .expect("failed to wait for subprocess");
/// }
///
/// execute(&["python", "-c", "print('Hello world!')"]);
/// ```
///
/// [`quote`]: fn.quote.html
/// [`split`]: fn.split.html
pub fn join<I, S>(words: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut line = words.into_iter().fold(String::new(), |mut line, word| {
        let quoted = quote(word.as_ref());
        line.push_str(quoted.as_ref());
        line.push(' ');
        line
    });
    line.pop();
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split_ok(cases: &[(&str, &[&str])]) {
        for &(input, expected) in cases {
            match split(input) {
                Err(actual) => {
                    panic!(
                        "After split({:?})\nexpected: Ok({:?})\n  actual: Err({:?})\n",
                        input, expected, actual
                    );
                }
                Ok(actual) => {
                    assert!(
                        expected == actual.as_slice(),
                        "After split({:?}).unwrap()\nexpected: {:?}\n  actual: {:?}\n",
                        input,
                        expected,
                        actual
                    );
                }
            }
        }
    }

    #[test]
    fn split_empty() {
        split_ok(&[("", &[])]);
    }

    #[test]
    fn split_initial_whitespace_is_removed() {
        split_ok(&[
            ("     a", &["a"]),
            ("\t\t\t\tbar", &["bar"]),
            ("\t \nc", &["c"]),
        ]);
    }

    #[test]
    fn split_trailing_whitespace_is_removed() {
        split_ok(&[
            ("a  ", &["a"]),
            ("b\t", &["b"]),
            ("c\t \n \n \n", &["c"]),
            ("d\n\n", &["d"]),
        ]);
    }

    #[test]
    fn split_carriage_return_is_not_special() {
        split_ok(&[("c\ra\r'\r'\r", &["c\ra\r\r\r"])]);
    }

    #[test]
    fn split_single_quotes() {
        split_ok(&[
            (r#"''"#, &[r#""#]),
            (r#"'a'"#, &[r#"a"#]),
            (r#"'\'"#, &[r#"\"#]),
            (r#"' \ '"#, &[r#" \ "#]),
            (r#"'#'"#, &[r#"#"#]),
        ]);
    }

    #[test]
    fn split_double_quotes() {
        split_ok(&[
            (r#""""#, &[""]),
            (r#""""""#, &[""]),
            (r#""a b c' d""#, &["a b c' d"]),
            (r#""\a""#, &["\\a"]),
            (r#""$""#, &["$"]),
            (r#""\$""#, &["$"]),
            (r#""`""#, &["`"]),
            (r#""\`""#, &["`"]),
            (r#""\"""#, &["\""]),
            (r#""\\""#, &["\\"]),
            ("\"\n\"", &["\n"]),
            ("\"\\\n\"", &[""]),
        ]);
    }

    #[test]
    fn split_unquoted() {
        split_ok(&[
            (r#"\|\&\;"#, &[r#"|&;"#]),
            (r#"\<\>"#, &[r#"<>"#]),
            (r#"\(\)"#, &[r#"()"#]),
            (r#"\$"#, &[r#"$"#]),
            (r#"\`"#, &[r#"`"#]),
            (r#"\""#, &[r#"""#]),
            (r#"\'"#, &[r#"'"#]),
            ("\\\n", &[]),
            (" \\\n \n", &[]),
            ("a\nb\nc", &["a", "b", "c"]),
            ("a\\\nb\\\nc", &["abc"]),
            ("foo bar baz", &["foo", "bar", "baz"]),
            (r#"\🦉"#, &[r"🦉"]),
        ]);
    }

    #[test]
    fn split_trailing_backslash() {
        split_ok(&[("\\", &["\\"]), (" \\", &["\\"]), ("a\\", &["a\\"])]);
    }

    #[test]
    fn split_errors() {
        assert_eq!(split("'abc"), Err(ParseError));
        assert_eq!(split("\""), Err(ParseError));
        assert_eq!(split("'\\"), Err(ParseError));
        assert_eq!(split("'\\"), Err(ParseError));
    }

    #[test]
    fn split_comments() {
        split_ok(&[
            (r#" x # comment "#, &["x"]),
            (r#" w1#w2 "#, &["w1#w2"]),
            (r#"'not really a # comment'"#, &["not really a # comment"]),
            (" a # very long comment \n b # another comment", &["a", "b"]),
        ]);
    }

    #[test]
    fn test_quote() {
        assert_eq!(quote(""), "''");
        assert_eq!(quote("'"), "''\\'''");
        assert_eq!(quote("abc"), "abc");
        assert_eq!(quote("a \n  b"), "'a \n  b'");
        assert_eq!(quote("X'\nY"), "'X'\\''\nY'");
    }

    #[test]
    fn test_join() {
        assert_eq!(join(&["a", "b", "c"]), "a b c");
        assert_eq!(join(&[" ", "$", "\n"]), "' ' '$' '\n'");
    }

    #[test]
    fn join_followed_by_split_is_identity() {
        let cases: Vec<&[&str]> = vec![
            &["a"],
            &["python", "-c", "print('Hello world!')"],
            &["echo", " arg with spaces ", "arg \' with \" quotes"],
            &["even newlines are quoted correctly\n", "\n", "\n\n\t "],
            &["$", "`test`"],
            &["cat", "~user/log*"],
            &["test", "'a \"b", "\"X'"],
            &["empty", "", "", ""],
        ];
        for argv in cases {
            let args = join(argv);
            assert_eq!(split(&args).unwrap(), argv);
        }
    }
}
