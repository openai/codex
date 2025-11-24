use matchers::Pattern;
use std::{
    cmp::Ordering,
    error::Error,
    fmt::{self, Write},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering::*},
        Arc,
    },
};

use super::{FieldMap, LevelFilter};
use tracing_core::field::{Field, Visit};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Match {
    pub(crate) name: String, // TODO: allow match patterns for names?
    pub(crate) value: Option<ValueMatch>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CallsiteMatch {
    pub(crate) fields: FieldMap<ValueMatch>,
    pub(crate) level: LevelFilter,
}

#[derive(Debug)]
pub(crate) struct SpanMatch {
    fields: FieldMap<(ValueMatch, AtomicBool)>,
    level: LevelFilter,
    has_matched: AtomicBool,
}

pub(crate) struct MatchVisitor<'a> {
    inner: &'a SpanMatch,
}

#[derive(Debug, Clone)]
pub(crate) enum ValueMatch {
    /// Matches a specific `bool` value.
    Bool(bool),
    /// Matches a specific `f64` value.
    F64(f64),
    /// Matches a specific `u64` value.
    U64(u64),
    /// Matches a specific `i64` value.
    I64(i64),
    /// Matches any `NaN` `f64` value.
    NaN,
    /// Matches any field whose `fmt::Debug` output is equal to a fixed string.
    Debug(MatchDebug),
    /// Matches any field whose `fmt::Debug` output matches a regular expression
    /// pattern.
    Pat(Box<MatchPattern>),
}

impl Eq for ValueMatch {}

impl PartialEq for ValueMatch {
    fn eq(&self, other: &Self) -> bool {
        use ValueMatch::*;
        match (self, other) {
            (Bool(a), Bool(b)) => a.eq(b),
            (F64(a), F64(b)) => {
                debug_assert!(!a.is_nan());
                debug_assert!(!b.is_nan());

                a.eq(b)
            }
            (U64(a), U64(b)) => a.eq(b),
            (I64(a), I64(b)) => a.eq(b),
            (NaN, NaN) => true,
            (Pat(a), Pat(b)) => a.eq(b),
            _ => false,
        }
    }
}

impl Ord for ValueMatch {
    fn cmp(&self, other: &Self) -> Ordering {
        use ValueMatch::*;
        match (self, other) {
            (Bool(this), Bool(that)) => this.cmp(that),
            (Bool(_), _) => Ordering::Less,

            (F64(this), F64(that)) => this
                .partial_cmp(that)
                .expect("`ValueMatch::F64` may not contain `NaN` values"),
            (F64(_), Bool(_)) => Ordering::Greater,
            (F64(_), _) => Ordering::Less,

            (NaN, NaN) => Ordering::Equal,
            (NaN, Bool(_)) | (NaN, F64(_)) => Ordering::Greater,
            (NaN, _) => Ordering::Less,

            (U64(this), U64(that)) => this.cmp(that),
            (U64(_), Bool(_)) | (U64(_), F64(_)) | (U64(_), NaN) => Ordering::Greater,
            (U64(_), _) => Ordering::Less,

            (I64(this), I64(that)) => this.cmp(that),
            (I64(_), Bool(_)) | (I64(_), F64(_)) | (I64(_), NaN) | (I64(_), U64(_)) => {
                Ordering::Greater
            }
            (I64(_), _) => Ordering::Less,

            (Pat(this), Pat(that)) => this.cmp(that),
            (Pat(_), _) => Ordering::Greater,

            (Debug(this), Debug(that)) => this.cmp(that),
            (Debug(_), _) => Ordering::Greater,
        }
    }
}

impl PartialOrd for ValueMatch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Matches a field's `fmt::Debug` output against a regular expression pattern.
///
/// This is used for matching all non-literal field value filters when regular
/// expressions are enabled.
#[derive(Debug, Clone)]
pub(crate) struct MatchPattern {
    pub(crate) matcher: Pattern,
    pattern: Arc<str>,
}

/// Matches a field's `fmt::Debug` output against a fixed string pattern.
///
/// This is used for matching all non-literal field value filters when regular
/// expressions are disabled.
#[derive(Debug, Clone)]
pub(crate) struct MatchDebug {
    pattern: Arc<str>,
}

/// Indicates that a field name specified in a filter directive was invalid.
#[derive(Clone, Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "env-filter")))]
pub struct BadName {
    name: String,
}

// === impl Match ===

impl Match {
    pub(crate) fn has_value(&self) -> bool {
        self.value.is_some()
    }

    // TODO: reference count these strings?
    pub(crate) fn name(&self) -> String {
        self.name.clone()
    }

    pub(crate) fn parse(s: &str, regex: bool) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut parts = s.split('=');
        let name = parts
            .next()
            .ok_or_else(|| BadName {
                name: "".to_string(),
            })?
            // TODO: validate field name
            .to_string();
        let value = parts
            .next()
            .map(|part| match regex {
                true => ValueMatch::parse_regex(part),
                false => Ok(ValueMatch::parse_non_regex(part)),
            })
            .transpose()?;
        Ok(Match { name, value })
    }
}

impl fmt::Display for Match {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.name, f)?;
        if let Some(ref value) = self.value {
            write!(f, "={}", value)?;
        }
        Ok(())
    }
}

impl Ord for Match {
    fn cmp(&self, other: &Self) -> Ordering {
        // Ordering for `Match` directives is based first on _whether_ a value
        // is matched or not. This is semantically meaningful --- we would
        // prefer to check directives that match values first as they are more
        // specific.
        let has_value = match (self.value.as_ref(), other.value.as_ref()) {
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            _ => Ordering::Equal,
        };
        // If both directives match a value, we fall back to the field names in
        // length + lexicographic ordering, and if these are equal as well, we
        // compare the match directives.
        //
        // This ordering is no longer semantically meaningful but is necessary
        // so that the directives can be stored in the `BTreeMap` in a defined
        // order.
        has_value
            .then_with(|| self.name.cmp(&other.name))
            .then_with(|| self.value.cmp(&other.value))
    }
}

impl PartialOrd for Match {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// === impl ValueMatch ===

fn value_match_f64(v: f64) -> ValueMatch {
    if v.is_nan() {
        ValueMatch::NaN
    } else {
        ValueMatch::F64(v)
    }
}

impl ValueMatch {
    /// Parse a `ValueMatch` that will match `fmt::Debug` fields using regular
    /// expressions.
    ///
    /// This returns an error if the string didn't contain a valid `bool`,
    /// `u64`, `i64`, or `f64` literal, and couldn't be parsed as a regular
    /// expression.
    #[allow(clippy::result_large_err)]
    fn parse_regex(s: &str) -> Result<Self, matchers::BuildError> {
        s.parse::<bool>()
            .map(ValueMatch::Bool)
            .or_else(|_| s.parse::<u64>().map(ValueMatch::U64))
            .or_else(|_| s.parse::<i64>().map(ValueMatch::I64))
            .or_else(|_| s.parse::<f64>().map(value_match_f64))
            .or_else(|_| {
                s.parse::<MatchPattern>()
                    .map(|p| ValueMatch::Pat(Box::new(p)))
            })
    }

    /// Parse a `ValueMatch` that will match `fmt::Debug` against a fixed
    /// string.
    ///
    /// This does *not* return an error, because any string that isn't a valid
    /// `bool`, `u64`, `i64`, or `f64` literal is treated as expected
    /// `fmt::Debug` output.
    fn parse_non_regex(s: &str) -> Self {
        s.parse::<bool>()
            .map(ValueMatch::Bool)
            .or_else(|_| s.parse::<u64>().map(ValueMatch::U64))
            .or_else(|_| s.parse::<i64>().map(ValueMatch::I64))
            .or_else(|_| s.parse::<f64>().map(value_match_f64))
            .unwrap_or_else(|_| ValueMatch::Debug(MatchDebug::new(s)))
    }
}

impl fmt::Display for ValueMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueMatch::Bool(ref inner) => fmt::Display::fmt(inner, f),
            ValueMatch::F64(ref inner) => fmt::Display::fmt(inner, f),
            ValueMatch::NaN => fmt::Display::fmt(&f64::NAN, f),
            ValueMatch::I64(ref inner) => fmt::Display::fmt(inner, f),
            ValueMatch::U64(ref inner) => fmt::Display::fmt(inner, f),
            ValueMatch::Debug(ref inner) => fmt::Display::fmt(inner, f),
            ValueMatch::Pat(ref inner) => fmt::Display::fmt(inner, f),
        }
    }
}

// === impl MatchPattern ===

impl FromStr for MatchPattern {
    type Err = matchers::BuildError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let matcher = s.parse::<Pattern>()?;
        Ok(Self {
            matcher,
            pattern: s.to_owned().into(),
        })
    }
}

impl fmt::Display for MatchPattern {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.pattern, f)
    }
}

impl AsRef<str> for MatchPattern {
    #[inline]
    fn as_ref(&self) -> &str {
        self.pattern.as_ref()
    }
}

impl MatchPattern {
    #[inline]
    fn str_matches(&self, s: &impl AsRef<str>) -> bool {
        self.matcher.matches(s)
    }

    #[inline]
    fn debug_matches(&self, d: &impl fmt::Debug) -> bool {
        self.matcher.debug_matches(d)
    }

    pub(super) fn into_debug_match(self) -> MatchDebug {
        MatchDebug {
            pattern: self.pattern,
        }
    }
}

impl PartialEq for MatchPattern {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl Eq for MatchPattern {}

impl PartialOrd for MatchPattern {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MatchPattern {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.pattern.cmp(&other.pattern)
    }
}

// === impl MatchDebug ===

impl MatchDebug {
    fn new(s: &str) -> Self {
        Self {
            pattern: s.to_owned().into(),
        }
    }

    #[inline]
    fn debug_matches(&self, d: &impl fmt::Debug) -> bool {
        // Naively, we would probably match a value's `fmt::Debug` output by
        // formatting it to a string, and then checking if the string is equal
        // to the expected pattern. However, this would require allocating every
        // time we want to match a field value against a `Debug` matcher, which
        // can be avoided.
        //
        // Instead, we implement `fmt::Write` for a type that, rather than
        // actually _writing_ the strings to something, matches them against the
        // expected pattern, and returns an error if the pattern does not match.
        struct Matcher<'a> {
            pattern: &'a str,
        }

        impl fmt::Write for Matcher<'_> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                // If the string is longer than the remaining expected string,
                // we know it won't match, so bail.
                if s.len() > self.pattern.len() {
                    return Err(fmt::Error);
                }

                // If the expected string begins with the string that was
                // written, we are still potentially a match. Advance the
                // position in the expected pattern to chop off the matched
                // output, and continue.
                if self.pattern.starts_with(s) {
                    self.pattern = &self.pattern[s.len()..];
                    return Ok(());
                }

                // Otherwise, the expected string doesn't include the string
                // that was written at the current position, so the `fmt::Debug`
                // output doesn't match! Return an error signalling that this
                // doesn't match.
                Err(fmt::Error)
            }
        }
        let mut matcher = Matcher {
            pattern: &self.pattern,
        };

        // Try to "write" the value's `fmt::Debug` output to a `Matcher`. This
        // returns an error if the `fmt::Debug` implementation wrote any
        // characters that did not match the expected pattern.
        write!(matcher, "{:?}", d).is_ok()
    }
}

impl fmt::Display for MatchDebug {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.pattern, f)
    }
}

impl AsRef<str> for MatchDebug {
    #[inline]
    fn as_ref(&self) -> &str {
        self.pattern.as_ref()
    }
}

impl PartialEq for MatchDebug {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl Eq for MatchDebug {}

impl PartialOrd for MatchDebug {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MatchDebug {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.pattern.cmp(&other.pattern)
    }
}

// === impl BadName ===

impl Error for BadName {}

impl fmt::Display for BadName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid field name `{}`", self.name)
    }
}

impl CallsiteMatch {
    pub(crate) fn to_span_match(&self) -> SpanMatch {
        let fields = self
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), (v.clone(), AtomicBool::new(false))))
            .collect();
        SpanMatch {
            fields,
            level: self.level,
            has_matched: AtomicBool::new(false),
        }
    }
}

impl SpanMatch {
    pub(crate) fn visitor(&self) -> MatchVisitor<'_> {
        MatchVisitor { inner: self }
    }

    #[inline]
    pub(crate) fn is_matched(&self) -> bool {
        if self.has_matched.load(Acquire) {
            return true;
        }
        self.is_matched_slow()
    }

    #[inline(never)]
    fn is_matched_slow(&self) -> bool {
        let matched = self
            .fields
            .values()
            .all(|(_, matched)| matched.load(Acquire));
        if matched {
            self.has_matched.store(true, Release);
        }
        matched
    }

    #[inline]
    pub(crate) fn filter(&self) -> Option<LevelFilter> {
        if self.is_matched() {
            Some(self.level)
        } else {
            None
        }
    }
}

impl Visit for MatchVisitor<'_> {
    fn record_f64(&mut self, field: &Field, value: f64) {
        match self.inner.fields.get(field) {
            Some((ValueMatch::NaN, ref matched)) if value.is_nan() => {
                matched.store(true, Release);
            }
            Some((ValueMatch::F64(ref e), ref matched))
                if (value - *e).abs() < f64::EPSILON =>
            {
                matched.store(true, Release);
            }
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        use std::convert::TryInto;

        match self.inner.fields.get(field) {
            Some((ValueMatch::I64(ref e), ref matched)) if value == *e => {
                matched.store(true, Release);
            }
            Some((ValueMatch::U64(ref e), ref matched)) if Ok(value) == (*e).try_into() => {
                matched.store(true, Release);
            }
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match self.inner.fields.get(field) {
            Some((ValueMatch::U64(ref e), ref matched)) if value == *e => {
                matched.store(true, Release);
            }
            _ => {}
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        match self.inner.fields.get(field) {
            Some((ValueMatch::Bool(ref e), ref matched)) if value == *e => {
                matched.store(true, Release);
            }
            _ => {}
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        match self.inner.fields.get(field) {
            Some((ValueMatch::Pat(ref e), ref matched)) if e.str_matches(&value) => {
                matched.store(true, Release);
            }
            Some((ValueMatch::Debug(ref e), ref matched)) if e.debug_matches(&value) => {
                matched.store(true, Release)
            }
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        match self.inner.fields.get(field) {
            Some((ValueMatch::Pat(ref e), ref matched)) if e.debug_matches(&value) => {
                matched.store(true, Release);
            }
            Some((ValueMatch::Debug(ref e), ref matched)) if e.debug_matches(&value) => {
                matched.store(true, Release)
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug)]
    #[allow(dead_code)]
    struct MyStruct {
        answer: usize,
        question: &'static str,
    }

    #[test]
    fn debug_struct_match() {
        let my_struct = MyStruct {
            answer: 42,
            question: "life, the universe, and everything",
        };

        let pattern = "MyStruct { answer: 42, question: \"life, the universe, and everything\" }";

        assert_eq!(
            format!("{:?}", my_struct),
            pattern,
            "`MyStruct`'s `Debug` impl doesn't output the expected string"
        );

        let matcher = MatchDebug {
            pattern: pattern.into(),
        };
        assert!(matcher.debug_matches(&my_struct))
    }

    #[test]
    fn debug_struct_not_match() {
        let my_struct = MyStruct {
            answer: 42,
            question: "what shall we have for lunch?",
        };

        let pattern = "MyStruct { answer: 42, question: \"life, the universe, and everything\" }";

        assert_eq!(
            format!("{:?}", my_struct),
            "MyStruct { answer: 42, question: \"what shall we have for lunch?\" }",
            "`MyStruct`'s `Debug` impl doesn't output the expected string"
        );

        let matcher = MatchDebug {
            pattern: pattern.into(),
        };
        assert!(!matcher.debug_matches(&my_struct))
    }
}
