use crate::filter::level::{self, LevelFilter};
#[cfg(not(feature = "smallvec"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use core::{cmp::Ordering, fmt, iter::FromIterator, slice, str::FromStr};
use tracing_core::{Level, Metadata};
/// Indicates that a string could not be parsed as a filtering directive.
#[derive(Debug)]
pub struct ParseError {
    kind: ParseErrorKind,
}

/// A directive which will statically enable or disable a given callsite.
///
/// Unlike a dynamic directive, this can be cached by the callsite.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct StaticDirective {
    pub(in crate::filter) target: Option<String>,
    pub(in crate::filter) field_names: Vec<String>,
    pub(in crate::filter) level: LevelFilter,
}

#[cfg(feature = "smallvec")]
pub(crate) type FilterVec<T> = smallvec::SmallVec<[T; 8]>;
#[cfg(not(feature = "smallvec"))]
pub(crate) type FilterVec<T> = Vec<T>;

#[derive(Debug, PartialEq, Clone)]
pub(in crate::filter) struct DirectiveSet<T> {
    directives: FilterVec<T>,
    pub(in crate::filter) max_level: LevelFilter,
}

pub(in crate::filter) trait Match {
    fn cares_about(&self, meta: &Metadata<'_>) -> bool;
    fn level(&self) -> &LevelFilter;
}

#[derive(Debug)]
enum ParseErrorKind {
    #[cfg(feature = "std")]
    Field(Box<dyn std::error::Error + Send + Sync>),
    Level(level::ParseError),
    Other(Option<&'static str>),
}

// === impl DirectiveSet ===

impl<T> DirectiveSet<T> {
    // this is only used by `env-filter`.
    #[cfg(all(feature = "std", feature = "env-filter"))]
    pub(crate) fn is_empty(&self) -> bool {
        self.directives.is_empty()
    }

    pub(crate) fn iter(&self) -> slice::Iter<'_, T> {
        self.directives.iter()
    }
}

impl<T: Ord> Default for DirectiveSet<T> {
    fn default() -> Self {
        Self {
            directives: FilterVec::new(),
            max_level: LevelFilter::OFF,
        }
    }
}

impl<T: Match + Ord> DirectiveSet<T> {
    pub(crate) fn directives(&self) -> impl Iterator<Item = &T> {
        self.directives.iter()
    }

    pub(crate) fn directives_for<'a>(
        &'a self,
        metadata: &'a Metadata<'a>,
    ) -> impl Iterator<Item = &'a T> + 'a {
        self.directives().filter(move |d| d.cares_about(metadata))
    }

    pub(crate) fn add(&mut self, directive: T) {
        // does this directive enable a more verbose level than the current
        // max? if so, update the max level.
        let level = *directive.level();
        if level > self.max_level {
            self.max_level = level;
        }
        // insert the directive into the vec of directives, ordered by
        // specificity (length of target + number of field filters). this
        // ensures that, when finding a directive to match a span or event, we
        // search the directive set in most specific first order.
        match self.directives.binary_search(&directive) {
            Ok(i) => self.directives[i] = directive,
            Err(i) => self.directives.insert(i, directive),
        }
    }

    #[cfg(test)]
    pub(in crate::filter) fn into_vec(self) -> FilterVec<T> {
        self.directives
    }
}

impl<T: Match + Ord> FromIterator<T> for DirectiveSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut this = Self::default();
        this.extend(iter);
        this
    }
}

impl<T: Match + Ord> Extend<T> for DirectiveSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for directive in iter.into_iter() {
            self.add(directive);
        }
    }
}

impl<T> IntoIterator for DirectiveSet<T> {
    type Item = T;

    #[cfg(feature = "smallvec")]
    type IntoIter = smallvec::IntoIter<[T; 8]>;
    #[cfg(not(feature = "smallvec"))]
    type IntoIter = vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.directives.into_iter()
    }
}

// === impl Statics ===

impl DirectiveSet<StaticDirective> {
    pub(crate) fn enabled(&self, meta: &Metadata<'_>) -> bool {
        let level = meta.level();
        match self.directives_for(meta).next() {
            Some(d) => d.level >= *level,
            None => false,
        }
    }

    /// Same as `enabled` above, but skips `Directive`'s with fields.
    pub(crate) fn target_enabled(&self, target: &str, level: &Level) -> bool {
        match self.directives_for_target(target).next() {
            Some(d) => d.level >= *level,
            None => false,
        }
    }

    pub(crate) fn directives_for_target<'a>(
        &'a self,
        target: &'a str,
    ) -> impl Iterator<Item = &'a StaticDirective> + 'a {
        self.directives()
            .filter(move |d| d.cares_about_target(target))
    }
}

// === impl StaticDirective ===

impl StaticDirective {
    pub(in crate::filter) fn new(
        target: Option<String>,
        field_names: Vec<String>,
        level: LevelFilter,
    ) -> Self {
        Self {
            target,
            field_names,
            level,
        }
    }

    pub(in crate::filter) fn cares_about_target(&self, to_check: &str) -> bool {
        // Does this directive have a target filter, and does it match the
        // metadata's target?
        if let Some(ref target) = self.target {
            if !to_check.starts_with(&target[..]) {
                return false;
            }
        }

        if !self.field_names.is_empty() {
            return false;
        }

        true
    }
}

impl Ord for StaticDirective {
    fn cmp(&self, other: &StaticDirective) -> Ordering {
        // We attempt to order directives by how "specific" they are. This
        // ensures that we try the most specific directives first when
        // attempting to match a piece of metadata.

        // First, we compare based on whether a target is specified, and the
        // lengths of those targets if both have targets.
        let ordering = self
            .target
            .as_ref()
            .map(String::len)
            .cmp(&other.target.as_ref().map(String::len))
            // Then we compare how many field names are matched by each directive.
            .then_with(|| self.field_names.len().cmp(&other.field_names.len()))
            // Finally, we fall back to lexicographical ordering if the directives are
            // equally specific. Although this is no longer semantically important,
            // we need to define a total ordering to determine the directive's place
            // in the BTreeMap.
            .then_with(|| {
                self.target
                    .cmp(&other.target)
                    .then_with(|| self.field_names[..].cmp(&other.field_names[..]))
            })
            .reverse();

        #[cfg(debug_assertions)]
        {
            if ordering == Ordering::Equal {
                debug_assert_eq!(
                    self.target, other.target,
                    "invariant violated: Ordering::Equal must imply a.target == b.target"
                );
                debug_assert_eq!(
                    self.field_names, other.field_names,
                    "invariant violated: Ordering::Equal must imply a.field_names == b.field_names"
                );
            }
        }

        ordering
    }
}

impl PartialOrd for StaticDirective {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Match for StaticDirective {
    fn cares_about(&self, meta: &Metadata<'_>) -> bool {
        // Does this directive have a target filter, and does it match the
        // metadata's target?
        if let Some(ref target) = self.target {
            if !meta.target().starts_with(&target[..]) {
                return false;
            }
        }

        if meta.is_event() && !self.field_names.is_empty() {
            let fields = meta.fields();
            for name in &self.field_names {
                if fields.field(name).is_none() {
                    return false;
                }
            }
        }

        true
    }

    fn level(&self) -> &LevelFilter {
        &self.level
    }
}

impl Default for StaticDirective {
    fn default() -> Self {
        StaticDirective {
            target: None,
            field_names: Vec::new(),
            level: LevelFilter::ERROR,
        }
    }
}

impl fmt::Display for StaticDirective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut wrote_any = false;
        if let Some(ref target) = self.target {
            fmt::Display::fmt(target, f)?;
            wrote_any = true;
        }

        if !self.field_names.is_empty() {
            f.write_str("[")?;

            let mut fields = self.field_names.iter();
            if let Some(field) = fields.next() {
                write!(f, "{{{}", field)?;
                for field in fields {
                    write!(f, ",{}", field)?;
                }
                f.write_str("}")?;
            }

            f.write_str("]")?;
            wrote_any = true;
        }

        if wrote_any {
            f.write_str("=")?;
        }

        fmt::Display::fmt(&self.level, f)
    }
}

impl FromStr for StaticDirective {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // This method parses a filtering directive in one of the following
        // forms:
        //
        // * `foo=trace` (TARGET=LEVEL)
        // * `foo[{bar,baz}]=info` (TARGET[{FIELD,+}]=LEVEL)
        // * `trace` (bare LEVEL)
        // * `foo` (bare TARGET)
        let mut split = s.split('=');
        let part0 = split
            .next()
            .ok_or_else(|| ParseError::msg("string must not be empty"))?;

        // Directive includes an `=`:
        // * `foo=trace`
        // * `foo[{bar}]=trace`
        // * `foo[{bar,baz}]=trace`
        if let Some(part1) = split.next() {
            if split.next().is_some() {
                return Err(ParseError::msg(
                    "too many '=' in filter directive, expected 0 or 1",
                ));
            }

            let mut split = part0.split("[{");
            let target = split.next().map(String::from);
            let mut field_names = Vec::new();
            // Directive includes fields:
            // * `foo[{bar}]=trace`
            // * `foo[{bar,baz}]=trace`
            if let Some(maybe_fields) = split.next() {
                if split.next().is_some() {
                    return Err(ParseError::msg(
                        "too many '[{' in filter directive, expected 0 or 1",
                    ));
                }

                if !maybe_fields.ends_with("}]") {
                    return Err(ParseError::msg("expected fields list to end with '}]'"));
                }

                let fields = maybe_fields
                    .trim_end_matches("}]")
                    .split(',')
                    .filter_map(|s| {
                        if s.is_empty() {
                            None
                        } else {
                            Some(String::from(s))
                        }
                    });
                field_names.extend(fields);
            };
            let level = part1.parse()?;
            return Ok(Self {
                level,
                field_names,
                target,
            });
        }

        // Okay, the part after the `=` was empty, the directive is either a
        // bare level or a bare target.
        // * `foo`
        // * `info`
        Ok(match part0.parse::<LevelFilter>() {
            Ok(level) => Self {
                level,
                target: None,
                field_names: Vec::new(),
            },
            Err(_) => Self {
                target: Some(String::from(part0)),
                level: LevelFilter::TRACE,
                field_names: Vec::new(),
            },
        })
    }
}

// === impl ParseError ===

impl ParseError {
    #[cfg(all(feature = "std", feature = "env-filter"))]
    pub(crate) fn new() -> Self {
        ParseError {
            kind: ParseErrorKind::Other(None),
        }
    }

    pub(crate) fn msg(s: &'static str) -> Self {
        ParseError {
            kind: ParseErrorKind::Other(Some(s)),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ParseErrorKind::Other(None) => f.pad("invalid filter directive"),
            ParseErrorKind::Other(Some(msg)) => write!(f, "invalid filter directive: {}", msg),
            ParseErrorKind::Level(ref l) => l.fmt(f),
            #[cfg(feature = "std")]
            ParseErrorKind::Field(ref e) => write!(f, "invalid field filter: {}", e),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {
    fn description(&self) -> &str {
        "invalid filter directive"
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            ParseErrorKind::Other(_) => None,
            ParseErrorKind::Level(ref l) => Some(l),
            ParseErrorKind::Field(ref n) => Some(n.as_ref()),
        }
    }
}

#[cfg(feature = "std")]
impl From<Box<dyn std::error::Error + Send + Sync>> for ParseError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self {
            kind: ParseErrorKind::Field(e),
        }
    }
}

impl From<level::ParseError> for ParseError {
    fn from(l: level::ParseError) -> Self {
        Self {
            kind: ParseErrorKind::Level(l),
        }
    }
}
