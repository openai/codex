//! [`core::fmt::DebugTuple`] reimplementation with
//! [`DebugTuple::finish_non_exhaustive()`] method.

use ::core;
use core::fmt::{Debug, Formatter, Result, Write};
use core::prelude::v1::*;

/// Same as [`core::fmt::DebugTuple`], but with
/// [`DebugTuple::finish_non_exhaustive()`] method.
#[must_use = "must eventually call `finish()` or `finish_non_exhaustive()` on \
              Debug builders"]
pub struct DebugTuple<'a, 'b: 'a> {
    fmt: &'a mut Formatter<'b>,
    result: Result,
    fields: usize,
    empty_name: bool,
}

/// Creates a new [`DebugTuple`].
pub fn debug_tuple<'a, 'b>(
    fmt: &'a mut Formatter<'b>,
    name: &str,
) -> DebugTuple<'a, 'b> {
    let result = fmt.write_str(name);
    DebugTuple {
        fmt,
        result,
        fields: 0,
        empty_name: name.is_empty(),
    }
}

impl<'a, 'b: 'a> DebugTuple<'a, 'b> {
    /// Adds a new field to the generated tuple struct output.
    ///
    /// # Example
    ///
    /// ```rust
    /// use core::fmt;
    /// use derive_more::__private::debug_tuple;
    ///
    /// struct Foo(i32, String);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         debug_tuple(fmt, "Foo")
    ///             .field(&self.0) // We add the first field.
    ///             .field(&self.1) // We add the second field.
    ///             .finish() // We're good to go!
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(10, "Hello World".to_string())),
    ///     "Foo(10, \"Hello World\")",
    /// );
    /// ```
    pub fn field(&mut self, value: &dyn Debug) -> &mut Self {
        self.result = self.result.and_then(|_| {
            if self.is_pretty() {
                if self.fields == 0 {
                    self.fmt.write_str("(\n")?;
                }

                let mut padded_formatter = Padded::new(self.fmt);
                padded_formatter.write_fmt(format_args!("{value:#?}"))?;
                padded_formatter.write_str(",\n")
            } else {
                let prefix = if self.fields == 0 { "(" } else { ", " };
                self.fmt.write_str(prefix)?;
                value.fmt(self.fmt)
            }
        });

        self.fields += 1;
        self
    }

    /// Finishes output and returns any error encountered.
    ///
    /// # Example
    ///
    /// ```
    /// use core::fmt;
    /// use derive_more::__private::debug_tuple;
    ///
    /// struct Foo(i32, String);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         debug_tuple(fmt, "Foo")
    ///             .field(&self.0)
    ///             .field(&self.1)
    ///             .finish() // You need to call it to "finish" the
    ///                       // tuple formatting.
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(10, "Hello World".to_string())),
    ///     "Foo(10, \"Hello World\")",
    /// );
    /// ```
    pub fn finish(&mut self) -> Result {
        if self.fields > 0 {
            self.result = self.result.and_then(|_| {
                if self.fields == 1 && self.empty_name && !self.is_pretty() {
                    self.fmt.write_str(",")?;
                }
                self.fmt.write_str(")")
            });
        }
        self.result
    }

    /// Marks the struct as non-exhaustive, indicating to the reader that there are some other
    /// fields that are not shown in the debug representation, and finishes output, returning any
    /// error encountered.
    ///
    /// # Example
    ///
    /// ```rust
    /// use core::fmt;
    /// use derive_more::__private::debug_tuple;
    ///
    /// struct Bar(i32, f32);
    ///
    /// impl fmt::Debug for Bar {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         debug_tuple(fmt, "Bar")
    ///             .field(&self.0)
    ///             .finish_non_exhaustive() // Show that some other field(s) exist.
    ///     }
    /// }
    ///
    /// assert_eq!(format!("{:?}", Bar(10, 1.0)), "Bar(10, ..)");
    /// ```
    pub fn finish_non_exhaustive(&mut self) -> Result {
        self.result = self.result.and_then(|_| {
            if self.fields > 0 {
                if self.is_pretty() {
                    let mut padded_formatter = Padded::new(self.fmt);
                    padded_formatter.write_str("..\n")?;
                    self.fmt.write_str(")")
                } else {
                    self.fmt.write_str(", ..)")
                }
            } else {
                self.fmt.write_str("(..)")
            }
        });
        self.result
    }

    fn is_pretty(&self) -> bool {
        self.fmt.alternate()
    }
}

/// Wrapper for a [`Formatter`] adding 4 spaces on newlines for inner pretty
/// printed [`Debug`] values.
struct Padded<'a, 'b> {
    formatter: &'a mut Formatter<'b>,
    on_newline: bool,
}

impl<'a, 'b> Padded<'a, 'b> {
    fn new(formatter: &'a mut Formatter<'b>) -> Self {
        Self {
            formatter,
            on_newline: true,
        }
    }
}

impl<'a, 'b> Write for Padded<'a, 'b> {
    fn write_str(&mut self, s: &str) -> Result {
        for s in s.split_inclusive('\n') {
            if self.on_newline {
                self.formatter.write_str("    ")?;
            }

            self.on_newline = s.ends_with('\n');
            self.formatter.write_str(s)?;
        }

        Ok(())
    }
}
