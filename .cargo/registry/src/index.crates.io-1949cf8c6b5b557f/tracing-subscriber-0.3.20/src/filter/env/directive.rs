pub(crate) use crate::filter::directive::{FilterVec, ParseError, StaticDirective};
use crate::filter::{
    directive::{DirectiveSet, Match},
    env::{field, FieldMap},
    level::LevelFilter,
};
use std::{cmp::Ordering, fmt, iter::FromIterator, str::FromStr};
use tracing_core::{span, Level, Metadata};

/// A single filtering directive.
// TODO(eliza): add a builder for programmatically constructing directives?
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(docsrs, doc(cfg(feature = "env-filter")))]
pub struct Directive {
    in_span: Option<String>,
    fields: Vec<field::Match>,
    pub(crate) target: Option<String>,
    pub(crate) level: LevelFilter,
}

/// A set of dynamic filtering directives.
pub(super) type Dynamics = DirectiveSet<Directive>;

/// A set of static filtering directives.
pub(super) type Statics = DirectiveSet<StaticDirective>;

pub(crate) type CallsiteMatcher = MatchSet<field::CallsiteMatch>;
pub(crate) type SpanMatcher = MatchSet<field::SpanMatch>;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MatchSet<T> {
    field_matches: FilterVec<T>,
    base_level: LevelFilter,
}

impl Directive {
    pub(super) fn has_name(&self) -> bool {
        self.in_span.is_some()
    }

    pub(super) fn has_fields(&self) -> bool {
        !self.fields.is_empty()
    }

    pub(super) fn to_static(&self) -> Option<StaticDirective> {
        if !self.is_static() {
            return None;
        }

        // TODO(eliza): these strings are all immutable; we should consider
        // `Arc`ing them to make this more efficient...
        let field_names = self.fields.iter().map(field::Match::name).collect();

        Some(StaticDirective::new(
            self.target.clone(),
            field_names,
            self.level,
        ))
    }

    fn is_static(&self) -> bool {
        !self.has_name() && !self.fields.iter().any(field::Match::has_value)
    }

    pub(super) fn is_dynamic(&self) -> bool {
        self.has_name() || self.has_fields()
    }

    pub(crate) fn field_matcher(&self, meta: &Metadata<'_>) -> Option<field::CallsiteMatch> {
        let fieldset = meta.fields();
        let fields = self
            .fields
            .iter()
            .filter_map(
                |field::Match {
                     ref name,
                     ref value,
                 }| {
                    if let Some(field) = fieldset.field(name) {
                        let value = value.as_ref().cloned()?;
                        Some(Ok((field, value)))
                    } else {
                        Some(Err(()))
                    }
                },
            )
            .collect::<Result<FieldMap<_>, ()>>()
            .ok()?;
        Some(field::CallsiteMatch {
            fields,
            level: self.level,
        })
    }

    pub(super) fn make_tables(
        directives: impl IntoIterator<Item = Directive>,
    ) -> (Dynamics, Statics) {
        // TODO(eliza): this could be made more efficient...
        let (dyns, stats): (Vec<Directive>, Vec<Directive>) =
            directives.into_iter().partition(Directive::is_dynamic);
        let statics = stats
            .into_iter()
            .filter_map(|d| d.to_static())
            .chain(dyns.iter().filter_map(Directive::to_static))
            .collect();
        (Dynamics::from_iter(dyns), statics)
    }

    pub(super) fn deregexify(&mut self) {
        for field in &mut self.fields {
            field.value = match field.value.take() {
                Some(field::ValueMatch::Pat(pat)) => {
                    Some(field::ValueMatch::Debug(pat.into_debug_match()))
                }
                x => x,
            }
        }
    }

    pub(super) fn parse(from: &str, regex: bool) -> Result<Self, ParseError> {
        let mut cur = Self {
            level: LevelFilter::TRACE,
            target: None,
            in_span: None,
            fields: Vec::new(),
        };

        #[derive(Debug)]
        enum ParseState {
            Start,
            LevelOrTarget { start: usize },
            Span { span_start: usize },
            Field { field_start: usize },
            Fields,
            Target,
            Level { level_start: usize },
            Complete,
        }

        use ParseState::*;
        let mut state = Start;
        for (i, c) in from.trim().char_indices() {
            state = match (state, c) {
                (Start, '[') => Span { span_start: i + 1 },
                (Start, c) if !['-', ':', '_'].contains(&c) && !c.is_alphanumeric() => {
                    return Err(ParseError::new())
                }
                (Start, _) => LevelOrTarget { start: i },
                (LevelOrTarget { start }, '=') => {
                    cur.target = Some(from[start..i].to_owned());
                    Level { level_start: i + 1 }
                }
                (LevelOrTarget { start }, '[') => {
                    cur.target = Some(from[start..i].to_owned());
                    Span { span_start: i + 1 }
                }
                (LevelOrTarget { start }, ',') => {
                    let (level, target) = match &from[start..] {
                        "" => (LevelFilter::TRACE, None),
                        level_or_target => match LevelFilter::from_str(level_or_target) {
                            Ok(level) => (level, None),
                            Err(_) => (LevelFilter::TRACE, Some(level_or_target.to_owned())),
                        },
                    };

                    cur.level = level;
                    cur.target = target;
                    Complete
                }
                (state @ LevelOrTarget { .. }, _) => state,
                (Target, '=') => Level { level_start: i + 1 },
                (Span { span_start }, ']') => {
                    cur.in_span = Some(from[span_start..i].to_owned());
                    Target
                }
                (Span { span_start }, '{') => {
                    cur.in_span = match &from[span_start..i] {
                        "" => None,
                        _ => Some(from[span_start..i].to_owned()),
                    };
                    Field { field_start: i + 1 }
                }
                (state @ Span { .. }, _) => state,
                (Field { field_start }, '}') => {
                    cur.fields.push(match &from[field_start..i] {
                        "" => return Err(ParseError::new()),
                        field => field::Match::parse(field, regex)?,
                    });
                    Fields
                }
                (Field { field_start }, ',') => {
                    cur.fields.push(match &from[field_start..i] {
                        "" => return Err(ParseError::new()),
                        field => field::Match::parse(field, regex)?,
                    });
                    Field { field_start: i + 1 }
                }
                (state @ Field { .. }, _) => state,
                (Fields, ']') => Target,
                (Level { level_start }, ',') => {
                    cur.level = match &from[level_start..i] {
                        "" => LevelFilter::TRACE,
                        level => LevelFilter::from_str(level)?,
                    };
                    Complete
                }
                (state @ Level { .. }, _) => state,
                _ => return Err(ParseError::new()),
            };
        }

        match state {
            LevelOrTarget { start } => {
                let (level, target) = match &from[start..] {
                    "" => (LevelFilter::TRACE, None),
                    level_or_target => match LevelFilter::from_str(level_or_target) {
                        Ok(level) => (level, None),
                        // Setting the target without the level enables every level for that target
                        Err(_) => (LevelFilter::TRACE, Some(level_or_target.to_owned())),
                    },
                };

                cur.level = level;
                cur.target = target;
            }
            Level { level_start } => {
                cur.level = match &from[level_start..] {
                    "" => LevelFilter::TRACE,
                    level => LevelFilter::from_str(level)?,
                };
            }
            Target | Complete => {}
            _ => return Err(ParseError::new()),
        };

        Ok(cur)
    }
}

impl Match for Directive {
    fn cares_about(&self, meta: &Metadata<'_>) -> bool {
        // Does this directive have a target filter, and does it match the
        // metadata's target?
        if let Some(ref target) = self.target {
            if !meta.target().starts_with(&target[..]) {
                return false;
            }
        }

        // Do we have a name filter, and does it match the metadata's name?
        // TODO(eliza): put name globbing here?
        if let Some(ref name) = self.in_span {
            if name != meta.name() {
                return false;
            }
        }

        // Does the metadata define all the fields that this directive cares about?
        let actual_fields = meta.fields();
        for expected_field in &self.fields {
            // Does the actual field set (from the metadata) contain this field?
            if actual_fields.field(&expected_field.name).is_none() {
                return false;
            }
        }

        true
    }

    fn level(&self) -> &LevelFilter {
        &self.level
    }
}

impl FromStr for Directive {
    type Err = ParseError;
    fn from_str(from: &str) -> Result<Self, Self::Err> {
        Directive::parse(from, true)
    }
}

impl Default for Directive {
    fn default() -> Self {
        Directive {
            level: LevelFilter::OFF,
            target: None,
            in_span: None,
            fields: Vec::new(),
        }
    }
}

impl PartialOrd for Directive {
    fn partial_cmp(&self, other: &Directive) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Directive {
    fn cmp(&self, other: &Directive) -> Ordering {
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
            // Next compare based on the presence of span names.
            .then_with(|| self.in_span.is_some().cmp(&other.in_span.is_some()))
            // Then we compare how many fields are defined by each
            // directive.
            .then_with(|| self.fields.len().cmp(&other.fields.len()))
            // Finally, we fall back to lexicographical ordering if the directives are
            // equally specific. Although this is no longer semantically important,
            // we need to define a total ordering to determine the directive's place
            // in the BTreeMap.
            .then_with(|| {
                self.target
                    .cmp(&other.target)
                    .then_with(|| self.in_span.cmp(&other.in_span))
                    .then_with(|| self.fields[..].cmp(&other.fields[..]))
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
                    self.in_span, other.in_span,
                    "invariant violated: Ordering::Equal must imply a.in_span == b.in_span"
                );
                debug_assert_eq!(
                    self.fields, other.fields,
                    "invariant violated: Ordering::Equal must imply a.fields == b.fields"
                );
            }
        }

        ordering
    }
}

impl fmt::Display for Directive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut wrote_any = false;
        if let Some(ref target) = self.target {
            fmt::Display::fmt(target, f)?;
            wrote_any = true;
        }

        if self.in_span.is_some() || !self.fields.is_empty() {
            f.write_str("[")?;

            if let Some(ref span) = self.in_span {
                fmt::Display::fmt(span, f)?;
            }

            let mut fields = self.fields.iter();
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

impl From<LevelFilter> for Directive {
    fn from(level: LevelFilter) -> Self {
        Self {
            level,
            ..Self::default()
        }
    }
}

impl From<Level> for Directive {
    fn from(level: Level) -> Self {
        LevelFilter::from_level(level).into()
    }
}

// === impl Dynamics ===

impl Dynamics {
    pub(crate) fn matcher(&self, metadata: &Metadata<'_>) -> Option<CallsiteMatcher> {
        let mut base_level = None;
        let field_matches = self
            .directives_for(metadata)
            .filter_map(|d| {
                if let Some(f) = d.field_matcher(metadata) {
                    return Some(f);
                }
                match base_level {
                    Some(ref b) if d.level > *b => base_level = Some(d.level),
                    None => base_level = Some(d.level),
                    _ => {}
                }
                None
            })
            .collect();

        if let Some(base_level) = base_level {
            Some(CallsiteMatcher {
                field_matches,
                base_level,
            })
        } else if !field_matches.is_empty() {
            Some(CallsiteMatcher {
                field_matches,
                base_level: base_level.unwrap_or(LevelFilter::OFF),
            })
        } else {
            None
        }
    }

    pub(crate) fn has_value_filters(&self) -> bool {
        self.directives()
            .any(|d| d.fields.iter().any(|f| f.value.is_some()))
    }
}

// ===== impl DynamicMatch =====

impl CallsiteMatcher {
    /// Create a new `SpanMatch` for a given instance of the matched callsite.
    pub(crate) fn to_span_match(&self, attrs: &span::Attributes<'_>) -> SpanMatcher {
        let field_matches = self
            .field_matches
            .iter()
            .map(|m| {
                let m = m.to_span_match();
                attrs.record(&mut m.visitor());
                m
            })
            .collect();
        SpanMatcher {
            field_matches,
            base_level: self.base_level,
        }
    }
}

impl SpanMatcher {
    /// Returns the level currently enabled for this callsite.
    pub(crate) fn level(&self) -> LevelFilter {
        self.field_matches
            .iter()
            .filter_map(field::SpanMatch::filter)
            .max()
            .unwrap_or(self.base_level)
    }

    pub(crate) fn record_update(&self, record: &span::Record<'_>) {
        for m in &self.field_matches {
            record.record(&mut m.visitor())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn parse_directives(dirs: impl AsRef<str>) -> Vec<Directive> {
        dirs.as_ref()
            .split(',')
            .filter_map(|s| s.parse().ok())
            .collect()
    }

    fn expect_parse(dirs: impl AsRef<str>) -> Vec<Directive> {
        dirs.as_ref()
            .split(',')
            .map(|s| {
                s.parse()
                    .unwrap_or_else(|err| panic!("directive '{:?}' should parse: {}", s, err))
            })
            .collect()
    }

    #[test]
    fn directive_ordering_by_target_len() {
        // TODO(eliza): it would be nice to have a property-based test for this
        // instead.
        let mut dirs = expect_parse(
            "foo::bar=debug,foo::bar::baz=trace,foo=info,a_really_long_name_with_no_colons=warn",
        );
        dirs.sort_unstable();

        let expected = vec![
            "a_really_long_name_with_no_colons",
            "foo::bar::baz",
            "foo::bar",
            "foo",
        ];
        let sorted = dirs
            .iter()
            .map(|d| d.target.as_ref().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(expected, sorted);
    }
    #[test]
    fn directive_ordering_by_span() {
        // TODO(eliza): it would be nice to have a property-based test for this
        // instead.
        let mut dirs = expect_parse("bar[span]=trace,foo=debug,baz::quux=info,a[span]=warn");
        dirs.sort_unstable();

        let expected = vec!["baz::quux", "bar", "foo", "a"];
        let sorted = dirs
            .iter()
            .map(|d| d.target.as_ref().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(expected, sorted);
    }

    #[test]
    fn directive_ordering_uses_lexicographic_when_equal() {
        // TODO(eliza): it would be nice to have a property-based test for this
        // instead.
        let mut dirs = expect_parse("span[b]=debug,b=debug,a=trace,c=info,span[a]=info");
        dirs.sort_unstable();

        let expected = vec![
            ("span", Some("b")),
            ("span", Some("a")),
            ("c", None),
            ("b", None),
            ("a", None),
        ];
        let sorted = dirs
            .iter()
            .map(|d| {
                (
                    d.target.as_ref().unwrap().as_ref(),
                    d.in_span.as_ref().map(String::as_ref),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(expected, sorted);
    }

    // TODO: this test requires the parser to support directives with multiple
    // fields, which it currently can't handle. We should enable this test when
    // that's implemented.
    #[test]
    #[ignore]
    fn directive_ordering_by_field_num() {
        // TODO(eliza): it would be nice to have a property-based test for this
        // instead.
        let mut dirs = expect_parse(
            "b[{foo,bar}]=info,c[{baz,quuux,quuux}]=debug,a[{foo}]=warn,bar[{field}]=trace,foo=debug,baz::quux=info"
        );
        dirs.sort_unstable();

        let expected = vec!["baz::quux", "bar", "foo", "c", "b", "a"];
        let sorted = dirs
            .iter()
            .map(|d| d.target.as_ref().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(expected, sorted);
    }

    #[test]
    fn parse_directives_ralith() {
        let dirs = parse_directives("common=trace,server=trace");
        assert_eq!(dirs.len(), 2, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("common".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::TRACE);
        assert_eq!(dirs[0].in_span, None);

        assert_eq!(dirs[1].target, Some("server".to_string()));
        assert_eq!(dirs[1].level, LevelFilter::TRACE);
        assert_eq!(dirs[1].in_span, None);
    }

    #[test]
    fn parse_directives_ralith_uc() {
        let dirs = parse_directives("common=INFO,server=DEBUG");
        assert_eq!(dirs.len(), 2, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("common".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::INFO);
        assert_eq!(dirs[0].in_span, None);

        assert_eq!(dirs[1].target, Some("server".to_string()));
        assert_eq!(dirs[1].level, LevelFilter::DEBUG);
        assert_eq!(dirs[1].in_span, None);
    }

    #[test]
    fn parse_directives_ralith_mixed() {
        let dirs = parse_directives("common=iNfo,server=dEbUg");
        assert_eq!(dirs.len(), 2, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("common".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::INFO);
        assert_eq!(dirs[0].in_span, None);

        assert_eq!(dirs[1].target, Some("server".to_string()));
        assert_eq!(dirs[1].level, LevelFilter::DEBUG);
        assert_eq!(dirs[1].in_span, None);
    }

    #[test]
    fn parse_directives_valid() {
        let dirs = parse_directives("crate1::mod1=error,crate1::mod2,crate2=debug,crate3=off");
        assert_eq!(dirs.len(), 4, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("crate1::mod1".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::ERROR);
        assert_eq!(dirs[0].in_span, None);

        assert_eq!(dirs[1].target, Some("crate1::mod2".to_string()));
        assert_eq!(dirs[1].level, LevelFilter::TRACE);
        assert_eq!(dirs[1].in_span, None);

        assert_eq!(dirs[2].target, Some("crate2".to_string()));
        assert_eq!(dirs[2].level, LevelFilter::DEBUG);
        assert_eq!(dirs[2].in_span, None);

        assert_eq!(dirs[3].target, Some("crate3".to_string()));
        assert_eq!(dirs[3].level, LevelFilter::OFF);
        assert_eq!(dirs[3].in_span, None);
    }

    #[test]

    fn parse_level_directives() {
        let dirs = parse_directives(
            "crate1::mod1=error,crate1::mod2=warn,crate1::mod2::mod3=info,\
             crate2=debug,crate3=trace,crate3::mod2::mod1=off",
        );
        assert_eq!(dirs.len(), 6, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("crate1::mod1".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::ERROR);
        assert_eq!(dirs[0].in_span, None);

        assert_eq!(dirs[1].target, Some("crate1::mod2".to_string()));
        assert_eq!(dirs[1].level, LevelFilter::WARN);
        assert_eq!(dirs[1].in_span, None);

        assert_eq!(dirs[2].target, Some("crate1::mod2::mod3".to_string()));
        assert_eq!(dirs[2].level, LevelFilter::INFO);
        assert_eq!(dirs[2].in_span, None);

        assert_eq!(dirs[3].target, Some("crate2".to_string()));
        assert_eq!(dirs[3].level, LevelFilter::DEBUG);
        assert_eq!(dirs[3].in_span, None);

        assert_eq!(dirs[4].target, Some("crate3".to_string()));
        assert_eq!(dirs[4].level, LevelFilter::TRACE);
        assert_eq!(dirs[4].in_span, None);

        assert_eq!(dirs[5].target, Some("crate3::mod2::mod1".to_string()));
        assert_eq!(dirs[5].level, LevelFilter::OFF);
        assert_eq!(dirs[5].in_span, None);
    }

    #[test]
    fn parse_uppercase_level_directives() {
        let dirs = parse_directives(
            "crate1::mod1=ERROR,crate1::mod2=WARN,crate1::mod2::mod3=INFO,\
             crate2=DEBUG,crate3=TRACE,crate3::mod2::mod1=OFF",
        );
        assert_eq!(dirs.len(), 6, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("crate1::mod1".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::ERROR);
        assert_eq!(dirs[0].in_span, None);

        assert_eq!(dirs[1].target, Some("crate1::mod2".to_string()));
        assert_eq!(dirs[1].level, LevelFilter::WARN);
        assert_eq!(dirs[1].in_span, None);

        assert_eq!(dirs[2].target, Some("crate1::mod2::mod3".to_string()));
        assert_eq!(dirs[2].level, LevelFilter::INFO);
        assert_eq!(dirs[2].in_span, None);

        assert_eq!(dirs[3].target, Some("crate2".to_string()));
        assert_eq!(dirs[3].level, LevelFilter::DEBUG);
        assert_eq!(dirs[3].in_span, None);

        assert_eq!(dirs[4].target, Some("crate3".to_string()));
        assert_eq!(dirs[4].level, LevelFilter::TRACE);
        assert_eq!(dirs[4].in_span, None);

        assert_eq!(dirs[5].target, Some("crate3::mod2::mod1".to_string()));
        assert_eq!(dirs[5].level, LevelFilter::OFF);
        assert_eq!(dirs[5].in_span, None);
    }

    #[test]
    fn parse_numeric_level_directives() {
        let dirs = parse_directives(
            "crate1::mod1=1,crate1::mod2=2,crate1::mod2::mod3=3,crate2=4,\
             crate3=5,crate3::mod2::mod1=0",
        );
        assert_eq!(dirs.len(), 6, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("crate1::mod1".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::ERROR);
        assert_eq!(dirs[0].in_span, None);

        assert_eq!(dirs[1].target, Some("crate1::mod2".to_string()));
        assert_eq!(dirs[1].level, LevelFilter::WARN);
        assert_eq!(dirs[1].in_span, None);

        assert_eq!(dirs[2].target, Some("crate1::mod2::mod3".to_string()));
        assert_eq!(dirs[2].level, LevelFilter::INFO);
        assert_eq!(dirs[2].in_span, None);

        assert_eq!(dirs[3].target, Some("crate2".to_string()));
        assert_eq!(dirs[3].level, LevelFilter::DEBUG);
        assert_eq!(dirs[3].in_span, None);

        assert_eq!(dirs[4].target, Some("crate3".to_string()));
        assert_eq!(dirs[4].level, LevelFilter::TRACE);
        assert_eq!(dirs[4].in_span, None);

        assert_eq!(dirs[5].target, Some("crate3::mod2::mod1".to_string()));
        assert_eq!(dirs[5].level, LevelFilter::OFF);
        assert_eq!(dirs[5].in_span, None);
    }

    #[test]
    fn parse_directives_invalid_crate() {
        // test parse_directives with multiple = in specification
        let dirs = parse_directives("crate1::mod1=warn=info,crate2=debug");
        assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("crate2".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::DEBUG);
        assert_eq!(dirs[0].in_span, None);
    }

    #[test]
    fn parse_directives_invalid_level() {
        // test parse_directives with 'noNumber' as log level
        let dirs = parse_directives("crate1::mod1=noNumber,crate2=debug");
        assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("crate2".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::DEBUG);
        assert_eq!(dirs[0].in_span, None);
    }

    #[test]
    fn parse_directives_string_level() {
        // test parse_directives with 'warn' as log level
        let dirs = parse_directives("crate1::mod1=wrong,crate2=warn");
        assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("crate2".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::WARN);
        assert_eq!(dirs[0].in_span, None);
    }

    #[test]
    fn parse_directives_empty_level() {
        // test parse_directives with '' as log level
        let dirs = parse_directives("crate1::mod1=wrong,crate2=");
        assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("crate2".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::TRACE);
        assert_eq!(dirs[0].in_span, None);
    }

    #[test]
    fn parse_directives_global() {
        // test parse_directives with no crate
        let dirs = parse_directives("warn,crate2=debug");
        assert_eq!(dirs.len(), 2, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, None);
        assert_eq!(dirs[0].level, LevelFilter::WARN);
        assert_eq!(dirs[1].in_span, None);

        assert_eq!(dirs[1].target, Some("crate2".to_string()));
        assert_eq!(dirs[1].level, LevelFilter::DEBUG);
        assert_eq!(dirs[1].in_span, None);
    }

    // helper function for tests below
    fn test_parse_bare_level(directive_to_test: &str, level_expected: LevelFilter) {
        let dirs = parse_directives(directive_to_test);
        assert_eq!(
            dirs.len(),
            1,
            "\ninput: \"{}\"; parsed: {:#?}",
            directive_to_test,
            dirs
        );
        assert_eq!(dirs[0].target, None);
        assert_eq!(dirs[0].level, level_expected);
        assert_eq!(dirs[0].in_span, None);
    }

    #[test]
    fn parse_directives_global_bare_warn_lc() {
        // test parse_directives with no crate, in isolation, all lowercase
        test_parse_bare_level("warn", LevelFilter::WARN);
    }

    #[test]
    fn parse_directives_global_bare_warn_uc() {
        // test parse_directives with no crate, in isolation, all uppercase
        test_parse_bare_level("WARN", LevelFilter::WARN);
    }

    #[test]
    fn parse_directives_global_bare_warn_mixed() {
        // test parse_directives with no crate, in isolation, mixed case
        test_parse_bare_level("wArN", LevelFilter::WARN);
    }

    #[test]
    fn parse_directives_valid_with_spans() {
        let dirs = parse_directives("crate1::mod1[foo]=error,crate1::mod2[bar],crate2[baz]=debug");
        assert_eq!(dirs.len(), 3, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("crate1::mod1".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::ERROR);
        assert_eq!(dirs[0].in_span, Some("foo".to_string()));

        assert_eq!(dirs[1].target, Some("crate1::mod2".to_string()));
        assert_eq!(dirs[1].level, LevelFilter::TRACE);
        assert_eq!(dirs[1].in_span, Some("bar".to_string()));

        assert_eq!(dirs[2].target, Some("crate2".to_string()));
        assert_eq!(dirs[2].level, LevelFilter::DEBUG);
        assert_eq!(dirs[2].in_span, Some("baz".to_string()));
    }

    #[test]
    fn parse_directives_with_dash_in_target_name() {
        let dirs = parse_directives("target-name=info");
        assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("target-name".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::INFO);
        assert_eq!(dirs[0].in_span, None);
    }

    #[test]
    fn parse_directives_with_dash_in_span_name() {
        // Reproduces https://github.com/tokio-rs/tracing/issues/1367

        let dirs = parse_directives("target[span-name]=info");
        assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("target".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::INFO);
        assert_eq!(dirs[0].in_span, Some("span-name".to_string()));
    }

    #[test]
    fn parse_directives_with_special_characters_in_span_name() {
        let span_name = "!\"#$%&'()*+-./:;<=>?@^_`|~[}";

        let dirs = parse_directives(format!("target[{}]=info", span_name));
        assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
        assert_eq!(dirs[0].target, Some("target".to_string()));
        assert_eq!(dirs[0].level, LevelFilter::INFO);
        assert_eq!(dirs[0].in_span, Some(span_name.to_string()));
    }

    #[test]
    fn parse_directives_with_invalid_span_chars() {
        let invalid_span_name = "]{";

        let dirs = parse_directives(format!("target[{}]=info", invalid_span_name));
        assert_eq!(dirs.len(), 0, "\nparsed: {:#?}", dirs);
    }
}
