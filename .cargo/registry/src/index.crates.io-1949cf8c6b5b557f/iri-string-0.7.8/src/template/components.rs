//! Syntax components of URI templates.

use core::mem;

use crate::parser::str::find_split_hole;
use crate::template::error::Error;
use crate::template::parser::validate as validate_parser;

/// Expression body.
///
/// This does not contain the wrapping braces (`{` and `}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExprBody<'a>(&'a str);

impl<'a> ExprBody<'a> {
    /// Creates a new expression body.
    ///
    /// # Precondition
    ///
    /// The given string should be a valid expression body.
    #[inline]
    #[must_use]
    pub(super) fn new(s: &'a str) -> Self {
        debug_assert!(
            !s.is_empty(),
            "[precondition] valid expression body is not empty"
        );

        Self(s)
    }

    /// Decomposes the expression into an `operator` and `variable-list`.
    ///
    /// # Panics
    ///
    /// May panic if the input is invalid.
    #[must_use]
    pub(super) fn decompose(&self) -> (Operator, VarListStr<'a>) {
        debug_assert!(
            !self.0.is_empty(),
            "[precondition] valid expression body is not empty"
        );
        let first = self.0.as_bytes()[0];
        if first.is_ascii_alphanumeric() || (first == b'_') || (first == b'%') {
            // The first byte is a part of the variable list.
            (Operator::String, VarListStr::new(self.0))
        } else {
            let op = Operator::from_byte(first).unwrap_or_else(|| {
                unreachable!(
                    "[precondition] valid expression has (optional) \
                     valid operator, but got a byte {first:#02x?}"
                )
            });
            (op, VarListStr::new(&self.0[1..]))
        }
    }

    /// Returns the raw expression in a string slice.
    #[inline]
    #[must_use]
    pub(super) fn as_str(&self) -> &'a str {
        self.0
    }
}

/// Variable name.
// QUESTION: Should hexdigits in percent-encoded triplets be compared case sensitively?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarName<'a>(&'a str);

impl<'a> VarName<'a> {
    /// Creates a `VarName` from the trusted string.
    ///
    /// # Precondition
    ///
    /// The given string should be a valid variable name.
    #[inline]
    #[must_use]
    pub(super) fn from_trusted(s: &'a str) -> Self {
        Self(s)
    }

    /// Creates a `VarName` from the string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::template::Error;
    /// use iri_string::template::context::VarName;
    ///
    /// let name = VarName::new("hello")?;
    /// assert_eq!(name.as_str(), "hello");
    ///
    /// assert!(VarName::new("0+non-variable-name").is_err());
    ///
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn new(s: &'a str) -> Result<Self, Error> {
        match validate_parser::validate_varname(s, 0) {
            Ok(_) => Ok(Self::from_trusted(s)),
            Err(e) => Err(e),
        }
    }

    /// Returns the varibale name.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &'a str {
        self.0
    }
}

/// Variable specifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarSpec<'a> {
    /// Variable name.
    name: VarName<'a>,
    /// Variable modifier.
    modifier: Modifier,
}

impl<'a> VarSpec<'a> {
    /// Returns the varibale name.
    #[inline]
    #[must_use]
    pub(super) fn name(&self) -> VarName<'a> {
        self.name
    }

    /// Returns the modifier.
    #[inline]
    #[must_use]
    pub(super) fn modifier(&self) -> Modifier {
        self.modifier
    }

    /// Parses the trusted varspec string.
    ///
    /// # Panics
    ///
    /// May panic if the input is invalid.
    #[must_use]
    pub(super) fn parse_trusted(s: &'a str) -> Self {
        if let Some(varname) = s.strip_suffix('*') {
            // `varname "*"`.
            return Self {
                name: VarName::from_trusted(varname),
                modifier: Modifier::Explode,
            };
        }
        // `varname ":" max-length` or `varname`.
        match find_split_hole(s, b':') {
            Some((varname, max_len)) => {
                let max_len: u16 = max_len
                    .parse()
                    .expect("[precondition] the input should be valid `varspec`");
                Self {
                    name: VarName::from_trusted(varname),
                    modifier: Modifier::MaxLen(max_len),
                }
            }
            None => Self {
                name: VarName(s),
                modifier: Modifier::None,
            },
        }
    }
}

/// Variable list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VarListStr<'a>(&'a str);

impl<'a> VarListStr<'a> {
    /// Creates a new variable list.
    ///
    /// # Precondition
    ///
    /// The given string should be a valid variable list.
    #[inline]
    #[must_use]
    pub(super) fn new(s: &'a str) -> Self {
        Self(s)
    }
}

impl<'a> IntoIterator for VarListStr<'a> {
    type IntoIter = VarListIter<'a>;
    type Item = (usize, VarSpec<'a>);

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        VarListIter { rest: self.0 }
    }
}

/// Iterator of variable specs.
#[derive(Debug, Clone)]
pub(super) struct VarListIter<'a> {
    /// Remaining input.
    rest: &'a str,
}

impl<'a> Iterator for VarListIter<'a> {
    /// A pair of the length of the varspec and the varspec itself.
    type Item = (usize, VarSpec<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        match find_split_hole(self.rest, b',') {
            Some((prefix, new_rest)) => {
                self.rest = new_rest;
                Some((prefix.len(), VarSpec::parse_trusted(prefix)))
            }
            None => {
                if self.rest.is_empty() {
                    None
                } else {
                    Some((
                        self.rest.len(),
                        VarSpec::parse_trusted(mem::take(&mut self.rest)),
                    ))
                }
            }
        }
    }
}

/// Variable modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Modifier {
    /// No modifiers.
    None,
    /// Max length, greater than 0 and less than 10000.
    MaxLen(u16),
    /// Explode the variable, e.g. the var spec has `*`.
    Explode,
}

/// Operator that is possibly reserved for future extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum MaybeOperator {
    /// Working operator.
    Operator(Operator),
    /// Reserved for future extensions.
    Reserved(OperatorReservedForFuture),
}

impl MaybeOperator {
    /// Returns the operator for the given character.
    pub(super) fn from_byte(b: u8) -> Option<Self> {
        match b {
            b'+' => Some(Self::Operator(Operator::Reserved)),
            b'#' => Some(Self::Operator(Operator::Fragment)),
            b'.' => Some(Self::Operator(Operator::Label)),
            b'/' => Some(Self::Operator(Operator::PathSegments)),
            b';' => Some(Self::Operator(Operator::PathParams)),
            b'?' => Some(Self::Operator(Operator::FormQuery)),
            b'&' => Some(Self::Operator(Operator::FormQueryCont)),
            b'=' => Some(Self::Reserved(OperatorReservedForFuture::Equals)),
            b',' => Some(Self::Reserved(OperatorReservedForFuture::Comma)),
            b'!' => Some(Self::Reserved(OperatorReservedForFuture::Exclamation)),
            b'@' => Some(Self::Reserved(OperatorReservedForFuture::AtSign)),
            b'|' => Some(Self::Reserved(OperatorReservedForFuture::Pipe)),
            _ => None,
        }
    }
}

/// Working operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Operator {
    /// No operator. String expansion.
    String,
    /// Reserved expansion by `+`.
    Reserved,
    /// Fragment expansion by `#`.
    Fragment,
    /// Label expansion by `.`.
    Label,
    /// Path segments by `/`.
    PathSegments,
    /// Path-style parameters by `;`.
    PathParams,
    /// Form-style query by `?`.
    FormQuery,
    /// Form-style query continuation by `&`.
    FormQueryCont,
}

impl Operator {
    /// Returns the operator for the given character.
    #[must_use]
    pub(super) fn from_byte(b: u8) -> Option<Self> {
        match b {
            b'+' => Some(Self::Reserved),
            b'#' => Some(Self::Fragment),
            b'.' => Some(Self::Label),
            b'/' => Some(Self::PathSegments),
            b';' => Some(Self::PathParams),
            b'?' => Some(Self::FormQuery),
            b'&' => Some(Self::FormQueryCont),
            _ => None,
        }
    }

    /// Returns the string length of the operator.
    #[inline]
    #[must_use]
    pub(super) const fn len(self) -> usize {
        if matches!(self, Self::String) {
            0
        } else {
            1
        }
    }
}

/// Operator reserved for future extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum OperatorReservedForFuture {
    /// Reserved `=` operator.
    Equals,
    /// Reserved `,` operator.
    Comma,
    /// Reserved `!` operator.
    Exclamation,
    /// Reserved `@` operator.
    AtSign,
    /// Reserved `|` operator.
    Pipe,
}
