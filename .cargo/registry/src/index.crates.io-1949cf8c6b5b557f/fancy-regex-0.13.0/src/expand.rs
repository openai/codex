use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::parse::{parse_decimal, parse_id};
use crate::{Captures, CompileError, Error, ParseError, Regex};

/// A set of options for expanding a template string using the contents
/// of capture groups.
#[derive(Debug)]
pub struct Expander {
    sub_char: char,
    open: &'static str,
    close: &'static str,
    allow_undelimited_name: bool,
}

impl Default for Expander {
    /// Returns the default expander used by [`Captures::expand`].
    ///
    /// [`Captures::expand`]: struct.Captures.html#expand
    fn default() -> Self {
        Expander {
            sub_char: '$',
            open: "{",
            close: "}",
            allow_undelimited_name: true,
        }
    }
}

impl Expander {
    /// Returns an expander that uses Python-compatible syntax.
    ///
    /// Expands all instances of `\num` or `\g<name>` in `replacement`
    /// to the corresponding capture group `num` or `name`, and writes
    /// them to the `dst` buffer given.
    ///
    /// `name` may be an integer corresponding to the index of the
    /// capture group (counted by order of opening parenthesis where `\0` is the
    /// entire match) or it can be a name (consisting of letters, digits or
    /// underscores) corresponding to a named capture group.
    ///
    /// `num` must be an integer corresponding to the index of the
    /// capture group.
    ///
    /// If `num` or `name` isn't a valid capture group (whether the name doesn't exist
    /// or isn't a valid index), then it is replaced with the empty string.
    ///
    /// The longest possible number is used. e.g., `\10` looks up capture
    /// group 10 and not capture group 1 followed by a literal 0.
    ///
    /// To write a literal `\`, use `\\`.
    pub fn python() -> Expander {
        Expander {
            sub_char: '\\',
            open: "g<",
            close: ">",
            allow_undelimited_name: false,
        }
    }

    /// Checks `template` for errors.  The following conditions are checked for:
    ///
    /// - A reference to a numbered group that does not exist in `regex`
    /// - A reference to a numbered group (other than 0) when `regex` contains named groups
    /// - A reference to a named group that does not occur in `regex`
    /// - An opening group name delimiter without a closing delimiter
    /// - Using an empty string as a group name
    pub fn check(&self, template: &str, regex: &Regex) -> crate::Result<()> {
        let on_group_num = |num| {
            if num == 0 {
                Ok(())
            } else if !regex.named_groups.is_empty() {
                Err(Error::CompileError(CompileError::NamedBackrefOnly))
            } else if num < regex.captures_len() {
                Ok(())
            } else {
                Err(Error::CompileError(CompileError::InvalidBackref))
            }
        };
        self.exec(template, |step| match step {
            Step::Char(_) => Ok(()),
            Step::GroupName(name) => {
                if regex.named_groups.contains_key(name) {
                    Ok(())
                } else if let Ok(num) = name.parse() {
                    on_group_num(num)
                } else {
                    Err(Error::CompileError(CompileError::InvalidBackref))
                }
            }
            Step::GroupNum(num) => on_group_num(num),
            Step::Error => Err(Error::ParseError(
                0,
                ParseError::GeneralParseError(
                    "parse error in template while expanding".to_string(),
                ),
            )),
        })
    }

    /// Escapes the substitution character in `text` so it appears literally
    /// in the output of `expansion`.
    ///
    /// ```
    /// assert_eq!(
    ///     fancy_regex::Expander::default().escape("Has a literal $ sign."),
    ///     "Has a literal $$ sign.",
    /// );
    /// ```
    pub fn escape<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if text.contains(self.sub_char) {
            let mut quoted = String::with_capacity(self.sub_char.len_utf8() * 2);
            quoted.push(self.sub_char);
            quoted.push(self.sub_char);
            Cow::Owned(text.replace(self.sub_char, &quoted))
        } else {
            Cow::Borrowed(text)
        }
    }

    #[doc(hidden)]
    #[deprecated(since = "0.4.0", note = "Use `escape` instead.")]
    pub fn quote<'a>(&self, text: &'a str) -> Cow<'a, str> {
        self.escape(text)
    }

    /// Expands the template string `template` using the syntax defined
    /// by this expander and the values of capture groups from `captures`.
    pub fn expansion(&self, template: &str, captures: &Captures<'_>) -> String {
        let mut cursor = Vec::with_capacity(template.len());
        #[cfg(feature = "std")]
        self.write_expansion(&mut cursor, template, captures)
            .expect("expansion succeeded");
        #[cfg(not(feature = "std"))]
        self.write_expansion_vec(&mut cursor, template, captures)
            .expect("expansion succeeded");
        String::from_utf8(cursor).expect("expansion is UTF-8")
    }

    /// Appends the expansion produced by `expansion` to `dst`.  Potentially more efficient
    /// than calling `expansion` directly and appending to an existing string.
    pub fn append_expansion(&self, dst: &mut String, template: &str, captures: &Captures<'_>) {
        let mut cursor = core::mem::take(dst).into_bytes();
        #[cfg(feature = "std")]
        self.write_expansion(&mut cursor, template, captures)
            .expect("expansion succeeded");
        #[cfg(not(feature = "std"))]
        self.write_expansion_vec(&mut cursor, template, captures)
            .expect("expansion succeeded");
        *dst = String::from_utf8(cursor).expect("expansion is UTF-8");
    }

    /// Writes the expansion produced by `expansion` to `dst`.  Potentially more efficient
    /// than calling `expansion` directly and writing the result.
    #[cfg(feature = "std")]
    pub fn write_expansion(
        &self,
        mut dst: impl std::io::Write,
        template: &str,
        captures: &Captures<'_>,
    ) -> std::io::Result<()> {
        self.exec(template, |step| match step {
            Step::Char(c) => write!(dst, "{}", c),
            Step::GroupName(name) => {
                if let Some(m) = captures.name(name) {
                    write!(dst, "{}", m.as_str())
                } else if let Some(m) = name.parse().ok().and_then(|num| captures.get(num)) {
                    write!(dst, "{}", m.as_str())
                } else {
                    Ok(())
                }
            }
            Step::GroupNum(num) => {
                if let Some(m) = captures.get(num) {
                    write!(dst, "{}", m.as_str())
                } else {
                    Ok(())
                }
            }
            Step::Error => Ok(()),
        })
    }

    /// Writes the expansion produced by `expansion` to `dst`.  Potentially more efficient
    /// than calling `expansion` directly and writing the result.
    pub fn write_expansion_vec(
        &self,
        dst: &mut Vec<u8>,
        template: &str,
        captures: &Captures<'_>,
    ) -> core::fmt::Result {
        self.exec(template, |step| match step {
            Step::Char(c) => Ok(dst.extend(c.to_string().as_bytes())),
            Step::GroupName(name) => {
                if let Some(m) = captures.name(name) {
                    Ok(dst.extend(m.as_str().as_bytes()))
                } else if let Some(m) = name.parse().ok().and_then(|num| captures.get(num)) {
                    Ok(dst.extend(m.as_str().as_bytes()))
                } else {
                    Ok(())
                }
            }
            Step::GroupNum(num) => {
                if let Some(m) = captures.get(num) {
                    Ok(dst.extend(m.as_str().as_bytes()))
                } else {
                    Ok(())
                }
            }
            Step::Error => Ok(()),
        })
    }

    fn exec<'t, E>(
        &self,
        template: &'t str,
        mut f: impl FnMut(Step<'t>) -> Result<(), E>,
    ) -> Result<(), E> {
        debug_assert!(!self.open.is_empty());
        debug_assert!(!self.close.is_empty());
        let mut iter = template.chars();
        while let Some(c) = iter.next() {
            if c == self.sub_char {
                let tail = iter.as_str();
                let skip = if tail.starts_with(self.sub_char) {
                    f(Step::Char(self.sub_char))?;
                    1
                } else if let Some((id, skip)) = parse_id(tail, self.open, self.close, false)
                    .or_else(|| {
                        if self.allow_undelimited_name {
                            parse_id(tail, "", "", false)
                        } else {
                            None
                        }
                    })
                {
                    f(Step::GroupName(id))?;
                    skip
                } else if let Some((skip, num)) = parse_decimal(tail, 0) {
                    f(Step::GroupNum(num))?;
                    skip
                } else {
                    f(Step::Error)?;
                    f(Step::Char(self.sub_char))?;
                    0
                };
                iter = iter.as_str()[skip..].chars();
            } else {
                f(Step::Char(c))?;
            }
        }
        Ok(())
    }
}

enum Step<'a> {
    Char(char),
    GroupName(&'a str),
    GroupNum(usize),
    Error,
}
