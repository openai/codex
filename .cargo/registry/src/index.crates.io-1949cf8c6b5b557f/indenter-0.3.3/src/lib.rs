//! A few wrappers for the `fmt::Write` objects that efficiently appends and remove
//! common indentation after every newline
//!
//! # Setup
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! indenter = "0.2"
//! ```
//!
//! # Examples
//!
//! ## Indentation only
//!
//! This type is intended primarily for writing error reporters that gracefully
//! format error messages that span multiple lines.
//!
//! ```rust
//! use std::error::Error;
//! use core::fmt::{self, Write};
//! use indenter::indented;
//!
//! struct ErrorReporter<'a>(&'a dyn Error);
//!
//! impl fmt::Debug for ErrorReporter<'_> {
//!     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!         let mut source = Some(self.0);
//!         let mut i = 0;
//!
//!         while let Some(error) = source {
//!             writeln!(f)?;
//!             write!(indented(f).ind(i), "{}", error)?;
//!
//!             source = error.source();
//!             i += 1;
//!         }
//!
//!         Ok(())
//!     }
//! }
//! ```
//!
//! ## "Dedenting" (removing common leading indendation)
//!
//! This type is intended primarily for formatting source code. For example, when
//! generating code.
//!
//! This type requires the feature `std`.
//!
//! ```rust
//! # #[cfg(feature = "std")]
//! # fn main() {
//! use std::error::Error;
//! use core::fmt::{self, Write};
//! use indenter::CodeFormatter;
//!
//! let mut output = String::new();
//! let mut f = CodeFormatter::new(&mut output, "    ");
//!
//! write!(
//!     f,
//!     r#"
//!     Hello
//!         World
//!     "#,
//! );
//!
//! assert_eq!(output, "Hello\n    World\n");
//!
//! let mut output = String::new();
//! let mut f = CodeFormatter::new(&mut output, "    ");
//!
//! // it can also indent...
//! f.indent(2);
//!
//! write!(
//!     f,
//!     r#"
//!     Hello
//!         World
//!     "#,
//! );
//!
//! assert_eq!(output, "        Hello\n            World\n");
//! # }
//! # #[cfg(not(feature = "std"))]
//! # fn main() {
//! # }
//! ```
#![cfg_attr(not(feature = "std"), no_std)]
#![doc(html_root_url = "https://docs.rs/indenter/0.3.3")]
#![warn(
    missing_debug_implementations,
    missing_docs,
    missing_doc_code_examples,
    rust_2018_idioms,
    unreachable_pub,
    bad_style,
    const_err,
    dead_code,
    improper_ctypes,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    private_in_public,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]
use core::fmt;

/// The set of supported formats for indentation
#[allow(missing_debug_implementations)]
pub enum Format<'a> {
    /// Insert uniform indentation before every line
    ///
    /// This format takes a static string as input and inserts it after every newline
    Uniform {
        /// The string to insert as indentation
        indentation: &'static str,
    },
    /// Inserts a number before the first line
    ///
    /// This format hard codes the indentation level to match the indentation from
    /// `core::backtrace::Backtrace`
    Numbered {
        /// The index to insert before the first line of output
        ind: usize,
    },
    /// A custom indenter which is executed after every newline
    ///
    /// Custom indenters are passed the current line number and the buffer to be written to as args
    Custom {
        /// The custom indenter
        inserter: &'a mut Inserter,
    },
}

/// Helper struct for efficiently indenting multi line display implementations
///
/// # Explanation
///
/// This type will never allocate a string to handle inserting indentation. It instead leverages
/// the `write_str` function that serves as the foundation of the `core::fmt::Write` trait. This
/// lets it intercept each piece of output as its being written to the output buffer. It then
/// splits on newlines giving slices into the original string. Finally we alternate writing these
/// lines and the specified indentation to the output buffer.
#[allow(missing_debug_implementations)]
pub struct Indented<'a, D: ?Sized> {
    inner: &'a mut D,
    needs_indent: bool,
    format: Format<'a>,
}

/// A callback for `Format::Custom` used to insert indenation after a new line
///
/// The first argument is the line number within the output, starting from 0
pub type Inserter = dyn FnMut(usize, &mut dyn fmt::Write) -> fmt::Result;

impl Format<'_> {
    fn insert_indentation(&mut self, line: usize, f: &mut dyn fmt::Write) -> fmt::Result {
        match self {
            Format::Uniform { indentation } => write!(f, "{}", indentation),
            Format::Numbered { ind } => {
                if line == 0 {
                    write!(f, "{: >4}: ", ind)
                } else {
                    write!(f, "      ")
                }
            }
            Format::Custom { inserter } => inserter(line, f),
        }
    }
}

impl<'a, D> Indented<'a, D> {
    /// Sets the format to `Format::Numbered` with the provided index
    pub fn ind(self, ind: usize) -> Self {
        self.with_format(Format::Numbered { ind })
    }

    /// Sets the format to `Format::Uniform` with the provided static string
    pub fn with_str(self, indentation: &'static str) -> Self {
        self.with_format(Format::Uniform { indentation })
    }

    /// Construct an indenter with a user defined format
    pub fn with_format(mut self, format: Format<'a>) -> Self {
        self.format = format;
        self
    }
}

impl<T> fmt::Write for Indented<'_, T>
where
    T: fmt::Write + ?Sized,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (ind, line) in s.split('\n').enumerate() {
            if ind > 0 {
                self.inner.write_char('\n')?;
                self.needs_indent = true;
            }

            if self.needs_indent {
                // Don't render the line unless its actually got text on it
                if line.is_empty() {
                    continue;
                }

                self.format.insert_indentation(ind, &mut self.inner)?;
                self.needs_indent = false;
            }

            self.inner.write_fmt(format_args!("{}", line))?;
        }

        Ok(())
    }
}

/// Helper function for creating a default indenter
pub fn indented<D: ?Sized>(f: &mut D) -> Indented<'_, D> {
    Indented {
        inner: f,
        needs_indent: true,
        format: Format::Uniform {
            indentation: "    ",
        },
    }
}

/// Helper struct for efficiently dedent and indent multi line display implementations
///
/// # Explanation
///
/// This type allocates a string once to get the formatted result and then uses the internal
/// formatter efficiently to: first dedent the output, then re-indent to the desired level.
#[cfg(feature = "std")]
#[allow(missing_debug_implementations)]
pub struct CodeFormatter<'a, T> {
    f: &'a mut T,
    level: u32,
    indentation: String,
}

#[cfg(feature = "std")]
impl<'a, T: fmt::Write> fmt::Write for CodeFormatter<'a, T> {
    fn write_str(&mut self, input: &str) -> fmt::Result {
        let input = match input.chars().next() {
            Some('\n') => &input[1..],
            _ => return self.f.write_str(input),
        };

        let min = input
            .split('\n')
            .map(|line| line.chars().take_while(char::is_ascii_whitespace).count())
            .filter(|count| *count > 0)
            .min()
            .unwrap_or_default();

        let input = input.trim_end_matches(|c| char::is_ascii_whitespace(&c));

        for line in input.split('\n') {
            if line.len().saturating_sub(min) > 0 {
                for _ in 0..self.level {
                    self.f.write_str(&self.indentation)?;
                }
            }

            if line.len() >= min {
                self.f.write_str(&line[min..])?;
            } else {
                self.f.write_str(&line)?;
            }
            self.f.write_char('\n')?;
        }

        Ok(())
    }

    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        self.write_str(&args.to_string())
    }
}

#[cfg(feature = "std")]
impl<'a, T: fmt::Write> CodeFormatter<'a, T> {
    /// Wrap the formatter `f`, use `indentation` as base string indentation and return a new
    /// formatter that implements `std::fmt::Write` that can be used with the macro `write!()`
    pub fn new<S: Into<String>>(f: &'a mut T, indentation: S) -> Self {
        Self {
            f,
            level: 0,
            indentation: indentation.into(),
        }
    }

    /// Set the indentation level to a specific value
    pub fn set_level(&mut self, level: u32) {
        self.level = level;
    }

    /// Increase the indentation level by `inc`
    pub fn indent(&mut self, inc: u32) {
        self.level = self.level.saturating_add(inc);
    }

    /// Decrease the indentation level by `inc`
    pub fn dedent(&mut self, inc: u32) {
        self.level = self.level.saturating_sub(inc);
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::string::String;
    use core::fmt::Write as _;

    #[test]
    fn one_digit() {
        let input = "verify\nthis";
        let expected = "   2: verify\n      this";
        let mut output = String::new();

        indented(&mut output).ind(2).write_str(input).unwrap();

        assert_eq!(expected, output);
    }

    #[test]
    fn two_digits() {
        let input = "verify\nthis";
        let expected = "  12: verify\n      this";
        let mut output = String::new();

        indented(&mut output).ind(12).write_str(input).unwrap();

        assert_eq!(expected, output);
    }

    #[test]
    fn no_digits() {
        let input = "verify\nthis";
        let expected = "    verify\n    this";
        let mut output = String::new();

        indented(&mut output).write_str(input).unwrap();

        assert_eq!(expected, output);
    }

    #[test]
    fn with_str() {
        let input = "verify\nthis";
        let expected = "...verify\n...this";
        let mut output = String::new();

        indented(&mut output)
            .with_str("...")
            .write_str(input)
            .unwrap();

        assert_eq!(expected, output);
    }

    #[test]
    fn dyn_write() {
        let input = "verify\nthis";
        let expected = "    verify\n    this";
        let mut output = String::new();
        let writer: &mut dyn core::fmt::Write = &mut output;

        indented(writer).write_str(input).unwrap();

        assert_eq!(expected, output);
    }

    #[test]
    fn nice_api() {
        let input = "verify\nthis";
        let expected = "   1: verify\n       this";
        let output = &mut String::new();
        let n = 1;

        write!(
            indented(output).with_format(Format::Custom {
                inserter: &mut move |line_no, f| {
                    if line_no == 0 {
                        write!(f, "{: >4}: ", n)
                    } else {
                        write!(f, "       ")
                    }
                }
            }),
            "{}",
            input
        )
        .unwrap();

        assert_eq!(expected, output);
    }

    #[test]
    fn nice_api_2() {
        let input = "verify\nthis";
        let expected = "  verify\n  this";
        let output = &mut String::new();

        write!(
            indented(output).with_format(Format::Uniform { indentation: "  " }),
            "{}",
            input
        )
        .unwrap();

        assert_eq!(expected, output);
    }

    #[test]
    fn trailing_newlines() {
        let input = "verify\nthis\n";
        let expected = "  verify\n  this\n";
        let output = &mut String::new();

        write!(indented(output).with_str("  "), "{}", input).unwrap();

        assert_eq!(expected, output);
    }

    #[test]
    fn several_interpolations() {
        let input = "verify\nthis\n";
        let expected = "  verify\n  this\n   and verify\n  this\n";
        let output = &mut String::new();

        write!(indented(output).with_str("  "), "{} and {}", input, input).unwrap();

        assert_eq!(expected, output);
    }
}

#[cfg(all(test, feature = "std"))]
mod tests_std {
    use super::*;
    use core::fmt::Write as _;

    #[test]
    fn dedent() {
        let mut s = String::new();
        let mut f = CodeFormatter::new(&mut s, "    ");
        write!(
            f,
            r#"
            struct Foo;

            impl Foo {{
                fn foo() {{
                    todo!()
                }}
            }}
            "#,
        )
        .unwrap();
        assert_eq!(
            s,
            "struct Foo;\n\nimpl Foo {\n    fn foo() {\n        todo!()\n    }\n}\n"
        );

        let mut s = String::new();
        let mut f = CodeFormatter::new(&mut s, "    ");
        write!(
            f,
            r#"
            struct Foo;

            impl Foo {{
                fn foo() {{
                    todo!()
                }}
            }}"#,
        )
        .unwrap();
        assert_eq!(
            s,
            "struct Foo;\n\nimpl Foo {\n    fn foo() {\n        todo!()\n    }\n}\n"
        );
    }

    #[test]
    fn indent() {
        let mut s = String::new();
        let mut f = CodeFormatter::new(&mut s, "    ");
        f.indent(1);
        write!(
            f,
            r#"
            struct Foo;

            impl Foo {{
                fn foo() {{
                    todo!()
                }}
            }}
            "#,
        )
        .unwrap();
        assert_eq!(s, "    struct Foo;\n\n    impl Foo {\n        fn foo() {\n            todo!()\n        }\n    }\n");
    }

    #[test]
    fn inline() {
        let mut s = String::new();
        let mut f = CodeFormatter::new(&mut s, "    ");
        write!(
            f,
            r#"struct Foo;
            fn foo() {{
            }}"#,
        )
        .unwrap();
        assert_eq!(s, "struct Foo;\n            fn foo() {\n            }");
    }

    #[test]
    fn split_prefix() {
        let mut s = String::new();
        let mut f = CodeFormatter::new(&mut s, "    ");
        writeln!(f).unwrap();
        assert_eq!(s, "\n");
    }
}
