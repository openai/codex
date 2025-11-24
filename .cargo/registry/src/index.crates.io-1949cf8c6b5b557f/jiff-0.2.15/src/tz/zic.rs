/*!
!!!!! THIS FILE IS NOT CURRENTLY INCLUDED IN JIFF !!!!!

WIP: I have dreams of providing a time zone database that could be backed by
tzdb's plain text files (which are the input to the `zic` TZif compiler).
But while I got as far as writing a complete parser for the format, actually
turning the data into something that can be used as a `TimeZone` has proven
quite difficult. I've read the code in `zic.c` quite a bit, but it's
essentially inscrutable. I think I could probably port it, but the problem is
that it's designed to convert these data types into a TZif file, where as I'd
more like to use them directly to implement the `TimeZone` methods. Howard
Hinnant's `tz` C++ library does this, but its implementation is quite tortured
as well.

The main complexity here is that in order to interpret any given AT field
(including a Zone's UNTIL field), you need to resolve any previous rules
in order to compute the correct SAVE offset. That's because AT (and UNTIL)
fields can be in "wall," "standard" or "universal" time, with "wall" being the
default and most common. In "wall" time, you need to know the right offset to
apply.

This seems also especially precarious for dealing with ambiguous (gaps & folds)
times. What happens when they straddle a Zone boundary for example?

My thinking here is that the right way to move forward is the following:

* A zic-backed time zone database would be a `ZicP` internally.
* When a specific time zone is requested, an in-memory TZif data type is
generated for just that time zone. We don't need to generate the TZif binary
data, but rather, use the `crate::tz::iana::binary::Tzif` data type directly.
* This includes generating POSIX TZ strings for future datetimes (which is
apparently not always possible since zic data is more expressive than what
POSIX TZ strings can represent).

If we can do that, then we're home free because `Tzif` does everything we want.
I think this is probably the right approach for the following reasons:

1. The zic format is clearly designed for this particular usage scenario.
So we'd be doing something for which there is known working (albeit in
inscrubtable C) code that accomplishes the same goal.
2. Time zone lookups would be much faster. That is, even if we figured out how
to adapt the zic data types below to do time zone lookups, they are likely
going to involve more overhead than what Tzif does. And if we were to try
and speed them up, it's likely we'd just wind up converging to the Tzif data
type anyway.

Basically, we need to write our own zic compiler to make this work. I decided
this was beyond the scope of what I wanted to do for the initial Jiff release,
so I abandoned this approach.

Also, why even bother with the plain text zic files in the first place? Well,
it's nice for a couple reasons:

1. It's a fair bit smaller than the full collection of binary files. Somewhere
around half the size.
2. They can be concatenated into one giant text file. And indeed, on some Unix
systems, `/usr/share/zoneinfo/tzdata.zi` is the complete database in plain
text format.
3. The plain text version has "links," which lets one determine the aliases for
various time zones. Although it seems like `backward` would help, it isn't
always included in `/usr/share/zoneinfo`. And, well, `tzdata.zi` isn't either,
for example, on macOS.

-----

This module provides support for zic plain text files from the [Time Zone
Database].

These text files are used by tzdb's `zic` compiler to produce the binary
TZif files commonly found in the `/usr/share/zoneinfo` directory of Unix
systems. The text files are sometimes found in the same directory. For example,
`/usr/share/zoneinfo/tzdata.zi` is a single file on my Archlinux system that
contains the entire set of time zones from tzdb. But my macOS system does not
have this file.

Generally speaking, the main benefit of the text file is that it is much
smaller. The entirety of tzdb can fit into a single 100KB zic file, where as
the corresponding binary data is at least 10 times as big. Moreover, the zic
files contain symbolic links between time zones such that it's possible to
determine whether two time zones are distinct or not.

The main down side of the zic formatted files is that they don't contain
pre-computed instants and are generally more complex to deal with. This in turn
means that offset lookups for both instants and civil datetimes are typically
slower than for a TZif backed time zone.

Note that the format parsed here is described in `man zic`.

# Organization

Types ending with `P` (for "parsed") are meant to be a structured
representation of the data from the zic file. The data at this point is
validated as much as possible, but it does not include all validation. For
example, a `ZoneFirstP` with a named reference to a set of rules that does not
exist is perfectly fine. But conversion to higher level non-`P` types will fail
in such cases.

[Time Zone Database]: https://www.iana.org/time-zones
*/

#![allow(warnings)]

use core::{ops::RangeInclusive, str::FromStr};

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::{
    civil::{Date, DateTime, Time, Weekday},
    error::{err, Error, ErrorContext},
    span::{Span, SpanFieldwise, ToSpan},
    timestamp::Timestamp,
    tz::{Dst, Offset},
    util::{
        parse,
        rangeint::RInto,
        sync::Arc,
        t::{self, C},
    },
    Unit,
};

#[derive(Debug, Default, Eq, PartialEq)]
struct Zic {
    zones: BTreeMap<String, Zone>,
    links: BTreeMap<String, Zone>,
}

impl Zic {
    fn new(zicp: ZicP) -> Result<Zic, Error> {
        todo!()
    }
}

#[derive(Debug, Eq, PartialEq)]
struct TimeZone {
    inner: Arc<TimeZoneInner>,
}

#[derive(Debug, Eq, PartialEq)]
struct TimeZoneInner {
    /// The canonical name for a time zone, according to tzdb. There can only
    /// be one of these, and it is the name with which the `Zone` line used.
    name: String,
    /// A list of zero or more aliases for this time zone created via `Link`
    /// lines.
    aliases: Vec<String>,
    /// A sequence of one or more "zones" that make up this time zone. Each
    /// zone is active until a certain point in time, at which some other zone
    /// takes over. Each zone can reference a distinct set of rules, and thus
    /// the specific zone to use for any given time dictates how DST is
    /// computed for that time.
    zones: Vec<Zone>,
}

#[derive(Debug, Eq, PartialEq)]
struct Zone {
    /// The offset to add to UTC to get standard time for this time zone.
    offset: Offset,
    /// The rules for determining the offset from standard time to use for this
    /// time zone.
    rules: Rules,
    /// The format to use when rendering a time zone abbreviation.
    format: ZoneFormatP,
    /// The timestamp until which this zone is active (exclusive).
    until_timestamp: Timestamp,
    /// The wall clock time until which this zone is active (exclusive).
    until_wall: DateTime,
}

#[derive(Debug, Eq, PartialEq)]
struct Rules {
    inner: Arc<RulesInner>,
}

#[derive(Debug, Eq, PartialEq)]
struct RulesInner {
    name: String,
    rules: Vec<Rule>,
}

impl Rules {
    /// Create a single rule group from a set of parsed rules.
    ///
    /// This can also return an error if converting from the more flexible
    /// parsed type to the more structured `Rule` fails in some way. For
    /// example, if the SAVE field has an offset bigger (or smaller) than what
    /// Jiff supports.
    ///
    /// # Panics
    ///
    /// Callers must ensure that the given group is non-empty and that every
    /// rule in the group has the same name.
    fn new(rulesp: Vec<RuleP>) -> Result<Rules, Error> {
        assert!(!rulesp.is_empty(), "rule group must be non-empty");
        let mut inner =
            RulesInner { name: rulesp[0].name.name.clone(), rules: vec![] };
        for r in rulesp {
            assert_eq!(
                inner.name, r.name.name,
                "every name in rule group must be identical"
            );
            let dst = Dst::from(r.save.suffix() == RuleSaveSuffixP::Dst);
            let offset = r.save.to_offset().map_err(|e| {
                err!("SAVE value in rule {:?} is too big: {e}", inner.name)
            })?;
            let years = r
                .years()
                .map_err(|e| e.context(err!("rule {:?}", inner.name)))?;
            let month = r.inn.month;
            let letters = r.letters.part;
            let day = r.on;
            let at = r.at;
            let rule = Rule { dst, offset, letters, years, month, day, at };
            inner.rules.push(rule);
        }
        Ok(Rules { inner: Arc::new(inner) })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Rule {
    dst: Dst,
    offset: Offset,
    letters: String,
    years: RangeInclusive<t::Year>,
    month: t::Month,
    day: RuleOnP,
    at: RuleAtP,
}

/// A collection of parsed lines from zero or more zic input files.
#[derive(Debug, Default, Eq, PartialEq)]
struct ZicP {
    rules: BTreeMap<String, Vec<RuleP>>,
    zones: BTreeMap<String, ZoneP>,
    links: BTreeMap<String, LinkP>,
}

impl ZicP {
    /// Parse the zic data given into this zic value.
    fn parse(&mut self, src: &str) -> Result<(), Error> {
        self.parse_with_fields(FieldParser::new(src))
    }

    /// Parse the zic data given into this zic value. If the data given isn't
    /// valid UTF-8, then this returns an error.
    fn parse_bytes(&mut self, src: &[u8]) -> Result<(), Error> {
        self.parse_with_fields(FieldParser::from_bytes(src)?)
    }

    /// Parse the zic data from the given field parser.
    fn parse_with_fields(
        &mut self,
        mut parser: FieldParser<'_>,
    ) -> Result<(), Error> {
        while parser.read_next_fields()? {
            self.parse_one(&mut parser)
                .map_err(|e| e.context(err!("line {}", parser.line_number)))?;
        }
        if let Some(ref name) = parser.continuation_zone_for {
            return Err(err!(
                "expected continuation zone line for {name:?}, \
                 but found end of data instead",
            ));
        }
        Ok(())
    }

    /// Parse a single line as a sequence of fields into this zic data.
    fn parse_one(&mut self, p: &mut FieldParser<'_>) -> Result<(), Error> {
        // Guaranteed by 'read_next_fields'
        assert!(!p.fields.is_empty());

        if let Some(name) = p.continuation_zone_for.take() {
            let zone = ZoneContinuationP::parse(&p.fields).map_err(|e| {
                e.context("failed to parse continuation 'Zone' line")
            })?;
            let more_continuations = zone.until.is_some();
            // OK because `p.continuation_zone_for` is only set when we have
            // seen a first zone with the corresponding name.
            self.zones.get_mut(&name).unwrap().continuations.push(zone);
            // Update our parser state if we expect another continuation line.
            if more_continuations {
                p.continuation_zone_for = Some(name);
            }
            return Ok(());
        }

        let (first, rest) = (&p.fields[0], &p.fields[1..]);
        if first.starts_with("R") && "Rule".starts_with(first) {
            let rule = RuleP::parse(rest)
                .map_err(|e| e.context("failed to parse 'Rule' line"))?;
            let name = rule.name.name.clone();
            self.rules.entry(name).or_default().push(rule);
        } else if first.starts_with("Z") && "Zone".starts_with(first) {
            let first = ZoneFirstP::parse(rest)
                .map_err(|e| e.context("failed to parse first 'Zone' line"))?;
            let name = first.name.name.clone();
            if first.until.is_some() {
                p.continuation_zone_for = Some(name.clone());
            }
            let zone = ZoneP { first, continuations: vec![] };
            if self.links.contains_key(&name) {
                return Err(err!(
                    "found zone with name {name:?} that conflicts \
                     with a link of the same name",
                ));
            }
            if let Some(previous_zone) = self.zones.insert(name, zone) {
                return Err(err!(
                    "found duplicate zone for {:?}",
                    previous_zone.first.name.name,
                ));
            }
        } else if first.starts_with("L") && "Link".starts_with(first) {
            let link = LinkP::parse(rest)
                .map_err(|e| e.context("failed to parse 'Link' line"))?;
            let name = link.name.name.clone();
            if self.zones.contains_key(&name) {
                return Err(err!(
                    "found link with name {name:?} that conflicts \
                     with a zone of the same name",
                ));
            }
            if let Some(previous_link) = self.links.insert(name, link) {
                return Err(err!(
                    "found duplicate link for {:?}",
                    previous_link.name.name,
                ));
            }
            // N.B. We don't check that the link's target name refers to some
            // other zone/link here, because the corresponding zone/link might
            // be defined later.
        } else {
            return Err(err!("unrecognized zic line: {first:?}"));
        }
        Ok(())
    }
}

/// A rule that determines when a particular amount of time should be added to
/// a zone's standard time.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RuleP {
    /// The name of this rule. The name can be referenced by zero or more
    /// zones in their RULES field. It is guaranteed to be non-empty.
    name: RuleNameP,
    /// The year at which this rule begins, inclusive.
    from: RuleFromP,
    /// The year at which this rule ends, inclusive.
    to: RuleToP,
    /// The month at which this rule becomes active.
    inn: RuleInP,
    /// The day of the month at which this rule becomes active.
    on: RuleOnP,
    /// The time of day at which this rule becomes active.
    at: RuleAtP,
    /// The amount of time to add to standard time when this rule is active.
    save: RuleSaveP,
    /// The latters that make up the variable part of a zone's abbreviation.
    letters: RuleLettersP,
}

impl RuleP {
    fn parse(fields: &[&str]) -> Result<RuleP, Error> {
        if fields.len() != 9 {
            return Err(err!(
                "expected exactly 9 fields for rule, but found {} fields",
                fields.len(),
            ));
        }
        let (name_field, fields) = (fields[0], &fields[1..]);
        let (from_field, fields) = (fields[0], &fields[1..]);
        let (to_field, fields) = (fields[0], &fields[1..]);
        let (_reserved_field, fields) = (fields[0], &fields[1..]);
        let (in_field, fields) = (fields[0], &fields[1..]);
        let (on_field, fields) = (fields[0], &fields[1..]);
        let (at_field, fields) = (fields[0], &fields[1..]);
        let (save_field, fields) = (fields[0], &fields[1..]);
        let (letters_field, fields) = (fields[0], &fields[1..]);

        let name = name_field
            .parse::<RuleNameP>()
            .map_err(|e| e.context("failed to parse NAME field"))?;
        let from = from_field
            .parse::<RuleFromP>()
            .map_err(|e| e.context("failed to parse FROM field"))?;
        let to = to_field
            .parse::<RuleToP>()
            .map_err(|e| e.context("failed to parse TO field"))?;
        let inn = in_field
            .parse::<RuleInP>()
            .map_err(|e| e.context("failed to parse IN field"))?;
        let on = on_field
            .parse::<RuleOnP>()
            .map_err(|e| e.context("failed to parse ON field"))?;
        let at = at_field
            .parse::<RuleAtP>()
            .map_err(|e| e.context("failed to parse AT field"))?;
        let save = save_field
            .parse::<RuleSaveP>()
            .map_err(|e| e.context("failed to parse SAVE field"))?;
        let letters = letters_field
            .parse::<RuleLettersP>()
            .map_err(|e| e.context("failed to parse LETTERS field"))?;

        Ok(RuleP { name, from, to, inn, on, at, save, letters })
    }

    fn years(&self) -> Result<RangeInclusive<t::Year>, Error> {
        let start = self.from.year;
        let end = match self.to {
            RuleToP::Max => t::Year::MAX_SELF,
            RuleToP::Only => start,
            RuleToP::Year { year } => year,
        };
        if start > end {
            return Err(err!(
                "found start year {start} to be greater than end year {end}"
            ));
        }
        Ok(start..=end)
    }
}

/// A group of one or more `Zone` lines.
///
/// A group of zones alwasys starts with a `Zone` line that has a name, and is
/// followed by zero or more continuation `Zone` lines.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ZoneP {
    /// The first zone line, alwasys present.
    first: ZoneFirstP,
    /// All continuation lines, may be empty.
    continuations: Vec<ZoneContinuationP>,
}

/// The parser representation of a `Zone` line.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ZoneFirstP {
    /// The name of the zone.
    name: ZoneNameP,
    /// The offset to add to UTC to get "standard" time for this zone.
    stdoff: ZoneStdoffP,
    /// The rules to apply for this zone. When `None`, standard time always
    /// applies.
    rules: ZoneRulesP,
    /// The format to use when rendering a time zone abbreviation.
    format: ZoneFormatP,
    /// The zone is active until this time.
    ///
    /// When present, it implies the existence of a continuation zone following
    /// this one. When absent, it is the last zone for the given name.
    until: Option<ZoneUntilP>,
}

impl ZoneFirstP {
    fn parse(fields: &[&str]) -> Result<ZoneFirstP, Error> {
        if fields.len() < 4 {
            return Err(err!("first ZONE line must have at least 4 fields"));
        }
        let (name_field, fields) = (fields[0], &fields[1..]);
        let (stdoff_field, fields) = (fields[0], &fields[1..]);
        let (rules_field, fields) = (fields[0], &fields[1..]);
        let (format_field, fields) = (fields[0], &fields[1..]);
        let name = name_field
            .parse::<ZoneNameP>()
            .map_err(|e| e.context("failed to parse NAME field"))?;
        let stdoff = stdoff_field
            .parse::<ZoneStdoffP>()
            .map_err(|e| e.context("failed to parse STDOFF field"))?;
        let rules = rules_field
            .parse::<ZoneRulesP>()
            .map_err(|e| e.context("failed to parse RULES field"))?;
        let format = format_field
            .parse::<ZoneFormatP>()
            .map_err(|e| e.context("failed to parse FORMAT field"))?;
        let until = if fields.is_empty() {
            None
        } else {
            Some(
                ZoneUntilP::parse(fields)
                    .map_err(|e| e.context("failed to parse UNTIL field"))?,
            )
        };
        Ok(ZoneFirstP { name, stdoff, rules, format, until })
    }
}

/// The parser representation of a `Zone` line.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ZoneContinuationP {
    /// The offset to add to UTC to get "standard" time for this zone.
    stdoff: ZoneStdoffP,
    /// The rules to apply for this zone. When `None`, standard time always
    /// applies.
    rules: ZoneRulesP,
    /// The format to use when rendering a time zone abbreviation.
    format: ZoneFormatP,
    /// The zone is active until this time.
    ///
    /// When present, it implies the existence of a continuation zone following
    /// this one. When absent, it is the last zone line for a particular name.
    until: Option<ZoneUntilP>,
}

impl ZoneContinuationP {
    fn parse(fields: &[&str]) -> Result<ZoneContinuationP, Error> {
        if fields.len() < 3 {
            return Err(err!(
                "continuation ZONE line must have at least 3 fields"
            ));
        }
        let (stdoff_field, fields) = (fields[0], &fields[1..]);
        let (rules_field, fields) = (fields[0], &fields[1..]);
        let (format_field, fields) = (fields[0], &fields[1..]);
        let stdoff = stdoff_field
            .parse::<ZoneStdoffP>()
            .map_err(|e| e.context("failed to parse STDOFF field"))?;
        let rules = rules_field
            .parse::<ZoneRulesP>()
            .map_err(|e| e.context("failed to parse RULES field"))?;
        let format = format_field
            .parse::<ZoneFormatP>()
            .map_err(|e| e.context("failed to parse FORMAT field"))?;
        let until = if fields.is_empty() {
            None
        } else {
            Some(
                ZoneUntilP::parse(fields)
                    .map_err(|e| e.context("failed to parse UNTIL field"))?,
            )
        };
        Ok(ZoneContinuationP { stdoff, rules, format, until })
    }
}

/// The parsed representation of a `Link` line.
#[derive(Clone, Debug, Eq, PartialEq)]
struct LinkP {
    /// The time zone being linked. This must be the name of some other `Zone`
    /// or `Link`.
    target: ZoneNameP,
    /// The name of the link.
    name: ZoneNameP,
}

impl LinkP {
    fn parse(fields: &[&str]) -> Result<LinkP, Error> {
        if fields.len() != 2 {
            return Err(err!(
                "expected exactly 2 fields after LINK, but found {}",
                fields.len()
            ));
        }
        let target = fields[0]
            .parse::<ZoneNameP>()
            .map_err(|e| e.context("failed to parse LINK target"))?;
        let name = fields[1]
            .parse::<ZoneNameP>()
            .map_err(|e| e.context("failed to parse LINK name"))?;
        Ok(LinkP { target, name })
    }
}

/// The name of a rule.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RuleNameP {
    name: String,
}

impl FromStr for RuleNameP {
    type Err = Error;

    fn from_str(name: &str) -> Result<RuleNameP, Error> {
        // The `man zic` docs say:
        //
        // > The name must start with a character that is neither an ASCII
        // > digit nor "-" nor "+". To allow for future extensions, an
        // > unquoted name should not contain characters from the set
        // > "!$%&'()*,/:;<=>?@[\]^`{|}~".
        //
        // We check that it doesn't start with [0-9+-] but don't bother
        // checking whether the name contains any of the other special
        // characters anywhere. It seems fine to skip that for now, and because
        // we don't actually know at this point whether the field was quoted
        // or not. We erase that information. We could rejigger things to keep
        // that information around, but... Meh.
        if name.is_empty() {
            Err(err!("NAME field for rule cannot be empty"))
        } else if name.starts_with(|ch| matches!(ch, '0'..='9' | '+' | '-')) {
            Err(err!(
                "NAME field cannot begin with a digit, + or -, \
                 but {name:?} begins with one of those",
            ))
        } else {
            Ok(RuleNameP { name: name.to_string() })
        }
    }
}

/// The year at which this rule begins (inclusive).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuleFromP {
    year: t::Year,
}

impl FromStr for RuleFromP {
    type Err = Error;

    fn from_str(from: &str) -> Result<RuleFromP, Error> {
        let year = parse_year(from)
            .map_err(|e| e.context("failed to parse FROM field"))?;
        Ok(RuleFromP { year })
    }
}

/// The year at which this rule ends (inclusive).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuleToP {
    /// The indefinite future.
    Max,
    /// Repeat the year given in the FROM field.
    Only,
    /// A specific year at which the rules ends. The year is an inclusive
    /// bound, but must be greater than or equal to the year in the FROM
    /// field of the rule.
    Year { year: t::Year },
}

impl FromStr for RuleToP {
    type Err = Error;

    fn from_str(to: &str) -> Result<RuleToP, Error> {
        if to.starts_with("m") && "maximum".starts_with(to) {
            Ok(RuleToP::Max)
        } else if to.starts_with("o") && "only".starts_with(to) {
            Ok(RuleToP::Only)
        } else {
            let year = parse_year(to)
                .map_err(|e| e.context("failed to parse TO field"))?;
            Ok(RuleToP::Year { year })
        }
    }
}

/// The month in which a rule becomes active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuleInP {
    month: t::Month,
}

impl FromStr for RuleInP {
    type Err = Error;

    fn from_str(field: &str) -> Result<RuleInP, Error> {
        static MONTH_PREFIXES: &[(u8, &str, &str)] = &[
            (1, "January", "Ja"),
            (2, "February", "F"),
            (3, "March", "Mar"),
            (4, "April", "Ap"),
            (5, "May", "May"),
            (6, "June", "Jun"),
            (7, "July", "Jul"),
            (8, "August", "Au"),
            (9, "September", "S"),
            (10, "October", "O"),
            (11, "November", "N"),
            (12, "December", "D"),
        ];
        for &(number, name, prefix) in MONTH_PREFIXES {
            if field.starts_with(prefix) && name.starts_with(field) {
                let month = t::Month::new(number).unwrap();
                return Ok(RuleInP { month });
            }
        }
        Err(err!("unrecognized month name: {field:?}"))
    }
}

/// The day of the month in which a rule becomes active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuleOnP {
    /// A specific fixed day of a month.
    Day { day: t::Day },
    /// The last weekday of a month.
    Last { weekday: Weekday },
    /// The weekday on or before a particular day of the month.
    OnOrBefore { weekday: Weekday, day: t::Day },
    /// The weekday on or after a particular day of the month.
    OnOrAfter { weekday: Weekday, day: t::Day },
}

impl RuleOnP {
    /// Given a year and a month, return the specific date for this "day of the
    /// month" specification.
    fn date(&self, year: t::Year, month: t::Month) -> Result<Date, Error> {
        match *self {
            RuleOnP::Day { day } => Date::new_ranged(year, month, day),
            RuleOnP::Last { weekday } => {
                // Always a valid month given that year/month are valid.
                let date =
                    Date::new_ranged(year, month, C(1).rinto()).unwrap();
                date.nth_weekday_of_month(-1, weekday)
            }
            RuleOnP::OnOrBefore { weekday, day } => {
                let start = Date::new_ranged(year, month, day)?
                    .checked_add(1.day())?;
                // nth_weekday returns "before" instead of "on or before," so
                // offset the date by a day to get "on or before" semantics.
                start.nth_weekday(-1, weekday)
            }
            RuleOnP::OnOrAfter { weekday, day } => {
                let start = Date::new_ranged(year, month, day)?
                    .checked_sub(1.day())?;
                // nth_weekday returns "after" instead of "on or after," so
                // offset the date by a day to get "on or after" semantics.
                start.nth_weekday(1, weekday)
            }
        }
    }
}

impl FromStr for RuleOnP {
    type Err = Error;

    fn from_str(field: &str) -> Result<RuleOnP, Error> {
        if field.starts_with("last") {
            let weekday = parse_weekday(&field[4..])?;
            Ok(RuleOnP::Last { weekday })
        } else if let Some(i) = field.find("<=") {
            let weekday = parse_weekday(&field[..i])?;
            let day = parse_day(&field[i + 2..])?;
            Ok(RuleOnP::OnOrBefore { weekday, day })
        } else if let Some(i) = field.find(">=") {
            let weekday = parse_weekday(&field[..i])?;
            let day = parse_day(&field[i + 2..])?;
            Ok(RuleOnP::OnOrAfter { weekday, day })
        } else if field.chars().all(|ch| ch.is_ascii_digit()) {
            let day = parse_day(field)?;
            // We don't check that `day` is valid for the month given in the IN
            // field. That gets checked at a higher level.
            Ok(RuleOnP::Day { day })
        } else {
            Err(err!("unrecognized format for day-of-month: {field:?}"))
        }
    }
}

/// The time of day at which a rule becomes active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuleAtP {
    /// The amount of time to add to the start of the day specified by
    /// `RuleOnP`. This may be negative.
    span: SpanFieldwise,
    /// An optional suffix indicating how to interpret the overall time at
    /// which a rule takes effect. As I understand it, this applies to the
    /// entire datetime that is specified by IN, ON and AT and not just the
    /// AT portion. (Because the suffix may cause the datetime to have an
    /// offset applied to it, and that offset can change the day!)
    ///
    /// When not present, wall clock time is assumed.
    suffix: Option<RuleAtSuffixP>,
}

impl RuleAtP {
    /// Returns the suffix for this AT field.
    ///
    /// When the suffix is absent, a default is selected.
    fn suffix(&self) -> RuleAtSuffixP {
        self.suffix.unwrap_or_default()
    }
}

impl FromStr for RuleAtP {
    type Err = Error;

    fn from_str(at: &str) -> Result<RuleAtP, Error> {
        if at.is_empty() {
            return Err(err!("empty field is not a valid AT value"));
        }
        let (span_string, suffix_string) = at.split_at(at.len() - 1);
        if suffix_string.chars().all(|ch| ch.is_ascii_alphabetic()) {
            let span = parse_span(span_string)?.fieldwise();
            let suffix = suffix_string.parse()?;
            Ok(RuleAtP { span, suffix: Some(suffix) })
        } else {
            let span = parse_span(at)?.fieldwise();
            Ok(RuleAtP { span, suffix: None })
        }
    }
}

/// The optional suffix that may be applied to the AT field of a rule.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum RuleAtSuffixP {
    #[default]
    Wall,
    Standard,
    Universal,
}

impl FromStr for RuleAtSuffixP {
    type Err = Error;

    fn from_str(suffix: &str) -> Result<RuleAtSuffixP, Error> {
        match suffix {
            "w" => Ok(RuleAtSuffixP::Wall),
            "s" => Ok(RuleAtSuffixP::Standard),
            "u" | "g" | "z" => Ok(RuleAtSuffixP::Universal),
            _ => Err(err!("unrecognized AT time suffix {suffix:?}")),
        }
    }
}

/// The amount of time to add to standard time when the corresponding rule is
/// in effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuleSaveP {
    /// The amount of time to add. This may be negative.
    span: SpanFieldwise,
    /// An optional suffix indicating how the resulting time after applying
    /// this rule should be interpreted. When absent, this defaults to DST.
    suffix: Option<RuleSaveSuffixP>,
}

impl RuleSaveP {
    /// Returns this "save" time as an offset.
    ///
    /// If the span is too long to fit in an offset, then an error is returned.
    fn to_offset(&self) -> Result<Offset, Error> {
        // TODO: I think `zic` rounds to the nearest second, breaking ties to
        // the nearest even second?
        let seconds = Span::from_invariant_nanoseconds(
            Unit::Second,
            self.span.0.to_invariant_nanoseconds(),
        )?
        .get_seconds();
        let seconds = i32::try_from(seconds).map_err(|_| {
            Error::range("SAVE seconds", seconds, i32::MIN, i32::MAX)
        })?;
        Offset::from_seconds(seconds)
    }

    /// Returns the suffix for this SAVE field.
    ///
    /// When the suffix is absent, a default is selected based on the time
    /// span in the field.
    fn suffix(&self) -> RuleSaveSuffixP {
        self.suffix.unwrap_or_else(|| {
            if self.span.0.is_zero() {
                RuleSaveSuffixP::Standard
            } else {
                RuleSaveSuffixP::Dst
            }
        })
    }
}

impl FromStr for RuleSaveP {
    type Err = Error;

    fn from_str(at: &str) -> Result<RuleSaveP, Error> {
        if at.is_empty() {
            return Err(err!("empty field is not a valid SAVE value"));
        }
        let (span_string, suffix_string) = at.split_at(at.len() - 1);
        if suffix_string.chars().all(|ch| ch.is_ascii_alphabetic()) {
            let span = parse_span(span_string)?.fieldwise();
            let suffix = suffix_string.parse()?;
            Ok(RuleSaveP { span, suffix: Some(suffix) })
        } else {
            let span = parse_span(at)?.fieldwise();
            Ok(RuleSaveP { span, suffix: None })
        }
    }
}

/// The optional suffix for the `SAVE` field of a `Rule` line.
///
/// The default for this depends on the time duration. When it's `0`, the
/// default suffix is `Standard`. Otherwise, it's `Dst`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuleSaveSuffixP {
    /// The resulting time after applying the corresponding rule should be
    /// treated as standard time.
    Standard,
    /// The resulting time after applying the corresponding rule should be
    /// treated as DST time.
    Dst,
}

impl FromStr for RuleSaveSuffixP {
    type Err = Error;

    fn from_str(suffix: &str) -> Result<RuleSaveSuffixP, Error> {
        match suffix {
            "s" => Ok(RuleSaveSuffixP::Standard),
            "d" => Ok(RuleSaveSuffixP::Dst),
            _ => Err(err!("unrecognized SAVE time suffix {suffix:?}")),
        }
    }
}

/// The latters that make up the variable part of a zone's abbreviation.
///
/// For example, if a zone's format is `E%sT`, then the letters might be
/// `S` or `D` for standard time and DST, respectively.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RuleLettersP {
    /// The actual value that should be interpolated. It may be absent, in
    /// which case, the empty string should be substituted.
    part: String,
}

impl FromStr for RuleLettersP {
    type Err = Error;

    fn from_str(letters: &str) -> Result<RuleLettersP, Error> {
        let part =
            if letters == "-" { String::new() } else { letters.to_string() };
        Ok(RuleLettersP { part })
    }
}

/// The name of a zone.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ZoneNameP {
    name: String,
}

impl FromStr for ZoneNameP {
    type Err = Error;

    fn from_str(name: &str) -> Result<ZoneNameP, Error> {
        if name.is_empty() {
            return Err(err!("zone names cannot be empty"));
        }
        for component in name.split('/') {
            if component == "." || component == ".." {
                return Err(err!(
                    "component {component:?} in zone name {name:?} cannot \
                     be \".\" or \"..\"",
                ));
            }
        }
        Ok(ZoneNameP { name: name.to_string() })
    }
}

/// The offset to add to UTC to get "standard" time for a zone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ZoneStdoffP {
    /// The duration. This only uses units of hours or lower.
    span: SpanFieldwise,
}

impl FromStr for ZoneStdoffP {
    type Err = Error;

    fn from_str(stdoff: &str) -> Result<ZoneStdoffP, Error> {
        let span = parse_span(stdoff)?.fieldwise();
        Ok(ZoneStdoffP { span })
    }
}

/// The rule specification for a zone.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ZoneRulesP {
    /// No rules are used. Standard time always applies.
    None,
    /// This zone uses a set of rules with the given name.
    Named(RuleNameP),
    /// An inline rule corresponding to a fixed offset from standard time for
    /// this zone.
    Save(RuleSaveP),
}

impl FromStr for ZoneRulesP {
    type Err = Error;

    fn from_str(rules: &str) -> Result<ZoneRulesP, Error> {
        if rules.starts_with(|ch: char| ch == '-' || ch.is_ascii_digit()) {
            // RuleSaveP actually accounts for the lone '-' case, so this is
            // technically redundant case analysis. But it is called out
            // specifically in the docs, so we parse it separately anyway.
            if rules == "-" {
                Ok(ZoneRulesP::None)
            } else {
                Ok(ZoneRulesP::Save(rules.parse()?))
            }
        } else {
            Ok(ZoneRulesP::Named(rules.parse()?))
        }
    }
}

/// The format of the abbreviation for this zone, including when it's in DST.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ZoneFormatP {
    /// This corresponds to a format that contains a `%s`, where the `%s` is
    /// meant to be interpolated with the letters specified in a matching rule.
    Variable {
        /// The text before a `%s`.
        before: String,
        /// The text after a `%s`.
        after: String,
    },
    /// This corresponds to the `%z` format, where the abbreviation should
    /// be a human readable rendering of the offset applied.
    Offset,
    /// This corresponds to the `STD/DST` format, where `STD` is the abbreviation
    /// for standard time, and `DST` is the abbreviation for DST.
    Pair {
        /// The abbreviation to use for standard time.
        std: String,
        /// The abbreviation to use for DST time.
        dst: String,
    },
    /// A static string that never changes.
    Static {
        /// The format string which is never interpolated.
        format: String,
    },
}

impl FromStr for ZoneFormatP {
    type Err = Error;

    fn from_str(format: &str) -> Result<ZoneFormatP, Error> {
        fn check_abbrev(abbrev: &str) -> Result<String, Error> {
            if abbrev.is_empty() {
                return Err(err!("empty abbreviations are not allowed"));
            }
            let is_ok =
                |ch| matches!(ch, '+'|'-'|'0'..='9'|'A'..='Z'|'a'..='z');
            if !abbrev.chars().all(is_ok) {
                return Err(err!(
                    "abbreviation {abbrev:?} \
                     contains invalid character; only \"+\", \"-\" and \
                     ASCII alpha-numeric characters are allowed"
                ));
            }
            Ok(abbrev.to_string())
        }
        if format == "%z" {
            Ok(ZoneFormatP::Offset)
        } else if let Some((before, after)) = format.split_once("%s") {
            Ok(ZoneFormatP::Variable {
                before: check_abbrev(before)?,
                after: check_abbrev(after)?,
            })
        } else if let Some((std, dst)) = format.split_once("/") {
            Ok(ZoneFormatP::Pair {
                std: check_abbrev(std)?,
                dst: check_abbrev(dst)?,
            })
        } else {
            Ok(ZoneFormatP::Static { format: check_abbrev(format)? })
        }
    }
}

/// The time until a particular zone is active.
///
/// This time is treated as an exclusive boundary. That is, times at precisely
/// this point are not governed by its corresponding zone.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ZoneUntilP {
    Year {
        year: t::Year,
    },
    YearMonth {
        year: t::Year,
        month: RuleInP,
    },
    YearMonthDay {
        year: t::Year,
        month: RuleInP,
        day: RuleOnP,
    },
    YearMonthDayTime {
        year: t::Year,
        month: RuleInP,
        day: RuleOnP,
        /// Note that adding a span to the year/month/day could overflow
        /// the allowed maximum time. This isn't handled until this type is
        /// converted into higher level data types.
        duration: RuleAtP,
    },
}

impl ZoneUntilP {
    fn parse(fields: &[&str]) -> Result<ZoneUntilP, Error> {
        if fields.is_empty() {
            return Err(err!("expected at least a year"));
        }

        let (year_field, fields) = (fields[0], &fields[1..]);
        let year = parse_year(year_field)
            .map_err(|e| e.context("failed to parse year"))?;
        if fields.is_empty() {
            return Ok(ZoneUntilP::Year { year });
        }

        let (month_field, fields) = (fields[0], &fields[1..]);
        let month = month_field
            .parse::<RuleInP>()
            .map_err(|e| e.context("failed to parse month"))?;
        if fields.is_empty() {
            return Ok(ZoneUntilP::YearMonth { year, month });
        }

        let (day_field, fields) = (fields[0], &fields[1..]);
        let day = day_field
            .parse::<RuleOnP>()
            .map_err(|e| e.context("failed to parse day"))?;
        if fields.is_empty() {
            return Ok(ZoneUntilP::YearMonthDay { year, month, day });
        }

        let (duration_field, fields) = (fields[0], &fields[1..]);
        let duration = duration_field
            .parse::<RuleAtP>()
            .map_err(|e| e.context("failed to parse time duration"))?;
        if !fields.is_empty() {
            return Err(err!(
                "expected no more fields after time of day, \
                 but found: {fields:?}",
                fields = fields.join(" "),
            ));
        }
        Ok(ZoneUntilP::YearMonthDayTime { year, month, day, duration })
    }

    fn to_datetime(&self) -> Result<DateTime, Error> {
        let date = self.on().date(self.year(), self.month())?;
        let dt = date
            .to_datetime(Time::midnight())
            .checked_add(self.at().span.0)?;
        Ok(dt)
    }

    fn year(&self) -> t::Year {
        use self::ZoneUntilP::*;

        match *self {
            Year { year }
            | YearMonth { year, .. }
            | YearMonthDay { year, .. }
            | YearMonthDayTime { year, .. } => year,
        }
    }

    fn month(&self) -> t::Month {
        use self::ZoneUntilP::*;

        match *self {
            Year { .. } => t::Month::N::<1>(),
            YearMonth { month, .. }
            | YearMonthDay { month, .. }
            | YearMonthDayTime { month, .. } => month.month,
        }
    }

    fn on(&self) -> RuleOnP {
        use self::ZoneUntilP::*;

        match *self {
            Year { .. } | YearMonth { .. } => {
                RuleOnP::Day { day: t::Day::N::<1>() }
            }
            YearMonthDay { day, .. } | YearMonthDayTime { day, .. } => day,
        }
    }

    fn at(&self) -> RuleAtP {
        use self::ZoneUntilP::*;

        match *self {
            Year { .. } | YearMonth { .. } | YearMonthDay { .. } => {
                RuleAtP { span: Span::new().fieldwise(), suffix: None }
            }
            YearMonthDayTime { duration, .. } => duration,
        }
    }
}

/// Parse a signed year value.
///
/// This ensures the year is within the range supported by Jiff.
fn parse_year(year: &str) -> Result<t::Year, Error> {
    let (sign, rest) = if year.starts_with("-") {
        (t::Sign::N::<-1>(), &year[1..])
    } else {
        (t::Sign::N::<1>(), year)
    };
    let number = parse::i64(rest.as_bytes())
        .map_err(|e| e.context("failed to parse year"))?;
    let year = t::Year::new(number)
        .ok_or_else(|| err!("year is out of range: {number}"))?;
    Ok(year * sign)
}

/// Parse a duration of time expressed in hours, minutes and seconds.
///
/// The format is something like: `[-]H+[:M+[:S+[.N+]]]`, where `H` is any
/// ASCII digit. Minutes and seconds are limited to the values `0-59`,
/// inclusive. Nanoseconds are limited to a fraction of a second. (And the
/// format doesn't actually specify a maximum precision, but we don't support
/// anything beyond nanoseconds, and in practice, the value will get rounded to
/// the nearest second at a higher level.) Hours can seemingly be any number we
/// can reasonably represent. The leading `-` sign means it can be negative.
/// And as a special case, a `-` all on its own is equivalent to `0`.
fn parse_span(span: &str) -> Result<Span, Error> {
    // This function is just brutal. I feel like I'm over-complicating it. I
    // suppoose we could probably do a little better here with a better set of
    // parser combinator helpers. But it's all inter-woven with parsing numbers
    // too.

    let rest = span;
    let (mut span, sign, rest) = if rest.starts_with("-") {
        // Special case where if the duration is just `-`, then it's equivalent
        // to zero.
        if span.len() == 1 {
            return Ok(Span::new());
        }
        (Span::new(), t::Sign::N::<-1>(), &rest[1..])
    } else {
        (Span::new(), t::Sign::N::<1>(), rest)
    };

    // Pluck out the hour component.
    let hour_len = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    let (hour_digits, rest) = rest.split_at(hour_len);
    if hour_digits.is_empty() {
        return Err(err!(
            "expected time duration to contain at least one hour digit"
        ));
    }
    let hours = parse::i64(hour_digits.as_bytes())
        .map_err(|e| e.context("failed to parse hours in time duration"))?;
    span = span
        .try_hours(hours.saturating_mul(i64::from(sign.get())))
        .map_err(|_| err!("duration hours '{hours:?}' is out of range"))?;
    if rest.is_empty() {
        return Ok(span);
    }

    // Now pluck out the minute component.
    if !rest.starts_with(":") {
        return Err(err!("expected ':' after hours, but found {rest:?}"));
    }
    let rest = &rest[1..];
    let minute_len = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    let (minute_digits, rest) = rest.split_at(minute_len);
    if minute_digits.is_empty() {
        return Err(err!(
            "expected minute digits after 'HH:', but found {rest:?} instead"
        ));
    }
    let minutes = parse::i64(minute_digits.as_bytes())
        .map_err(|e| e.context("failed to parse minutes in time duration"))?;
    let minutes_ranged = t::Minute::new(minutes).ok_or_else(|| {
        err!("duration minutes '{minutes:?}' is out of range")
    })?;
    span = span.minutes_ranged((minutes_ranged * sign).rinto());
    if rest.is_empty() {
        return Ok(span);
    }

    // Now pluck out the second component.
    if !rest.starts_with(":") {
        return Err(err!("expected ':' after minutes, but found {rest:?}"));
    }
    let rest = &rest[1..];
    let second_len = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    let (second_digits, rest) = rest.split_at(second_len);
    if second_digits.is_empty() {
        return Err(err!(
            "expected second digits after 'MM:', but found {rest:?} instead"
        ));
    }
    let seconds = parse::i64(second_digits.as_bytes())
        .map_err(|e| e.context("failed to parse seconds in time duration"))?;
    let seconds_ranged = t::Second::new(seconds).ok_or_else(|| {
        err!("duration seconds '{seconds:?}' is out of range")
    })?;
    span = span.seconds_ranged((seconds_ranged * sign).rinto());
    if rest.is_empty() {
        return Ok(span);
    }

    // Now look for the fractional nanosecond component.
    if !rest.starts_with(".") {
        return Err(err!("expected '.' after seconds, but found {rest:?}"));
    }
    let rest = &rest[1..];
    let nanosecond_len =
        rest.chars().take_while(|c| c.is_ascii_digit()).count();
    let (nanosecond_digits, rest) = rest.split_at(nanosecond_len);
    if nanosecond_digits.is_empty() {
        return Err(err!(
            "expected nanosecond digits after 'SS.', \
             but found {rest:?} instead"
        ));
    }
    let nanoseconds = parse::fraction(nanosecond_digits.as_bytes(), 9)
        .map_err(|e| {
            e.context("failed to parse nanoseconds in time duration")
        })?;
    let nanoseconds_ranged = t::FractionalNanosecond::new(nanoseconds)
        .ok_or_else(|| {
            err!("duration nanoseconds '{nanoseconds:?}' is out of range")
        })?;
    span = span.nanoseconds_ranged((nanoseconds_ranged * sign).rinto());

    // We should have consumed everything at this point.
    if !rest.is_empty() {
        return Err(err!(
            "found unrecognized trailing {rest:?} in time duration"
        ));
    }
    span.rebalance(Unit::Hour)
}

/// Parses a day of the month.
///
/// This checks that the day is in the range 1-31, but otherwise doesn't
/// check that it is valid for a particular month.
fn parse_day(string: &str) -> Result<t::Day, Error> {
    let number = parse::i64(string.as_bytes())
        .map_err(|e| e.context("failed to parse number for day"))?;
    let day = t::Day::new(number)
        .ok_or_else(|| err!("{number} is not a valid day"))?;
    Ok(day)
}

/// Parses a possibly abbreviated weekday from the given string.
fn parse_weekday(string: &str) -> Result<Weekday, Error> {
    static WEEKDAY_PREFIXES: &[(Weekday, &str, &str)] = &[
        (Weekday::Monday, "Monday", "M"),
        (Weekday::Tuesday, "Tuesday", "Tu"),
        (Weekday::Wednesday, "Wednesday", "W"),
        (Weekday::Thursday, "Thursday", "Th"),
        (Weekday::Friday, "Friday", "F"),
        (Weekday::Saturday, "Saturday", "Sa"),
        (Weekday::Sunday, "Sunday", "Su"),
    ];
    for &(weekday, name, prefix) in WEEKDAY_PREFIXES {
        if string.starts_with(prefix) && name.starts_with(string) {
            return Ok(weekday);
        }
    }
    Err(err!("unrecognized day of the week: {string:?}"))
}

/// A parser that emits lines as sequences of fields.
///
/// It is responsible for managing the state regarding whether to expect a
/// zone continuation line or not. It also knows to skip empty or commented
/// out lines.
struct FieldParser<'a> {
    /// The full underlying source data as an iterator of lines.
    lines: core::str::Lines<'a>,
    /// The current line number, starting at 1.
    line_number: usize,
    /// The fields from the current line, initially empty.
    fields: Vec<&'a str>,
    /// Set to the name of the zone when a continuation line for that zone is
    /// expected.
    continuation_zone_for: Option<String>,
}

impl<'a> FieldParser<'a> {
    /// Create a new parser from a UTF-8 encoded sequence of bytes.
    fn new(src: &'a str) -> FieldParser {
        FieldParser {
            lines: src.lines(),
            line_number: 0,
            fields: vec![],
            continuation_zone_for: None,
        }
    }

    /// Create a new parser from a sequence of bytes.
    ///
    /// This returns an error if the given bytes are not valid UTF-8.
    fn from_bytes(src: &'a [u8]) -> Result<FieldParser, Error> {
        let src = core::str::from_utf8(src)
            .map_err(|e| err!("invalid UTF-8: {e}"))?;
        Ok(FieldParser::new(src))
    }

    /// Advances the parser's line iterator and splits it into `self.fields`.
    ///
    /// If there are no more lines, then this returns `Ok(false)`. Otherwise,
    /// if the next line exists and was successfully parsed into a sequence of
    /// fields, then `Ok(true)` is returned.
    ///
    /// This guarantees that when `true` is returned, `self.fields` is
    /// non-empty.
    fn read_next_fields(&mut self) -> Result<bool, Error> {
        self.fields.clear();
        loop {
            let Some(mut line) = self.lines.next() else { return Ok(false) };
            self.line_number = self
                .line_number
                .checked_add(1)
                .ok_or_else(|| err!("line count overflowed"))?;
            parse_fields(&line, &mut self.fields)
                .with_context(|| err!("line {}", self.line_number))?;
            if self.fields.is_empty() {
                continue;
            }
            return Ok(true);
        }
    }
}

/// Splits the line given into fields.
///
/// This returns an error if there is an unclosed quoted field.
///
/// It is possible for this to return successfully without having written any
/// fields. Indeed, when an unquoted `#` appears, it and everything after it is
/// ignored.
///
/// Note that the specification in `man zic` for this is a little ambiguous.
/// For example, it doesn't quite say how data like `foo"bar baz"quux` should
/// be parsed. That is, it doesn't say that quoted fields must begin and end
/// with a quote. This parse will treat such things as two distinct unquoted
/// fields: `foo"bar` and `baz"quux`.
///
/// This always clears the `fields` given.
///
/// # Panics
///
/// This panics if a `\n` is seen while parsing the `line`.
fn parse_fields<'a>(
    mut line: &'a str,
    fields: &mut Vec<&'a str>,
) -> Result<(), Error> {
    /// Returns true if the given character corresponds to whitespace as
    /// defined in `man zic`.
    fn is_space(ch: char) -> bool {
        matches!(ch, ' ' | '\x0C' | '\n' | '\r' | '\t' | '\x0B')
    }

    // `man zic` says that the max line length including the line
    // terminator is 2048. The `core::str::Lines` iterator doesn't include
    // the terminator, so we subtract 1 to account for that. Note that this
    // could potentially allow one extra byte in the case of a \r\n line
    // terminator, but this seems fine.
    const MAX_LINE_LEN: usize = 2047;

    // The different possible states of the field parser below.
    enum State {
        Whitespace,
        InUnquote,
        InQuote,
        AfterQuote,
    }

    fields.clear();
    if line.len() > MAX_LINE_LEN {
        return Err(err!(
            "line with length {} exceeds \
             max length of {MAX_LINE_LEN}",
            line.len()
        ));
    }
    // Do a quick scan for a NUL terminator. They are illegal in all cases.
    if line.contains('\x00') {
        return Err(err!("found line with NUL byte, which isn't allowed"));
    }
    // The current state of the parser. We start at whitespace, since it also
    // means "before a field."
    let mut state = State::Whitespace;
    // The start of the current field. This is reset whenever we enter a state
    // corresponding to the beginning of a field's content.
    let mut start = 0;
    for (i, ch) in line.char_indices() {
        assert_ne!(ch, '\n', "no line terminators allowed within a line");
        state = match state {
            State::Whitespace => {
                if is_space(ch) {
                    State::Whitespace
                } else if ch == '#' {
                    return Ok(());
                } else if ch == '"' {
                    start = i + ch.len_utf8();
                    State::InQuote
                } else {
                    start = i;
                    State::InUnquote
                }
            }
            State::InUnquote => {
                if is_space(ch) {
                    fields.push(&line[start..i]);
                    State::Whitespace
                } else if ch == '#' {
                    fields.push(&line[start..i]);
                    return Ok(());
                } else {
                    State::InUnquote
                }
            }
            State::InQuote => {
                if ch == '"' {
                    fields.push(&line[start..i]);
                    State::AfterQuote
                } else {
                    State::InQuote
                }
            }
            State::AfterQuote => {
                if !is_space(ch) {
                    return Err(err!(
                        "expected whitespace after quoted field, \
                         but found {ch:?} instead",
                    ));
                }
                State::Whitespace
            }
        };
    }
    match state {
        State::Whitespace | State::AfterQuote => {}
        State::InUnquote => {
            fields.push(&line[start..]);
        }
        State::InQuote => {
            return Err(err!("found unclosed quote"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::civil::date;

    use super::*;

    fn td(seconds: i64, nanoseconds: i32) -> SpanFieldwise {
        Span::new()
            .seconds(seconds)
            .nanoseconds(nanoseconds)
            .rebalance(Unit::Hour)
            .unwrap()
            .fieldwise()
    }

    #[test]
    fn zone_until_to_datetime() {
        let until = ZoneUntilP::parse(&["2024"]).unwrap();
        let wall = until.to_datetime().unwrap();
        assert_eq!(wall, date(2024, 1, 1).at(0, 0, 0, 0));

        let until = ZoneUntilP::parse(&["2024", "Mar"]).unwrap();
        let wall = until.to_datetime().unwrap();
        assert_eq!(wall, date(2024, 3, 1).at(0, 0, 0, 0));

        let until = ZoneUntilP::parse(&["2024", "Mar", "Sun>=8"]).unwrap();
        let wall = until.to_datetime().unwrap();
        assert_eq!(wall, date(2024, 3, 10).at(0, 0, 0, 0));

        let until =
            ZoneUntilP::parse(&["2024", "Mar", "Sun>=8", "2:00"]).unwrap();
        let wall = until.to_datetime().unwrap();
        assert_eq!(wall, date(2024, 3, 10).at(2, 0, 0, 0));

        let until =
            ZoneUntilP::parse(&["2024", "Mar", "Sun>=10", "2:00"]).unwrap();
        let wall = until.to_datetime().unwrap();
        assert_eq!(wall, date(2024, 3, 10).at(2, 0, 0, 0));

        let until =
            ZoneUntilP::parse(&["2024", "Mar", "Sun<=10", "2:00"]).unwrap();
        let wall = until.to_datetime().unwrap();
        assert_eq!(wall, date(2024, 3, 10).at(2, 0, 0, 0));

        let until =
            ZoneUntilP::parse(&["2024", "Mar", "Sun<=10", "2:00u"]).unwrap();
        let wall = until.to_datetime().unwrap();
        assert_eq!(wall, date(2024, 3, 10).at(2, 0, 0, 0));
    }

    #[cfg(not(miri))]
    #[test]
    fn parse_zic_man1() {
        // An example from `man zic`.
        let data = "
         # Rule  NAME  FROM  TO    -  IN   ON       AT    SAVE  LETTER/S
         Rule    US    1967  2006  -  Oct  lastSun  2:00  0     S
         Rule    US    1967  1973  -  Apr  lastSun  2:00  1:00  D
         # Zone  NAME               STDOFF  RULES  FORMAT  [UNTIL]
         Zone    America/Menominee  -5:00   -      EST     1973 Apr 29 2:00
                 -6:00              US      C%sT
        ";
        let mut zic = ZicP::default();
        zic.parse(data).unwrap();
        insta::assert_debug_snapshot!(zic);
    }

    #[cfg(not(miri))]
    #[test]
    fn parse_zic_man2() {
        // An example from `man zic`.
        let data = "
         # Rule  NAME  FROM  TO    -  IN   ON       AT    SAVE  LETTER/S
         Rule    Swiss 1941  1942  -  May  Mon>=1   1:00  1:00  S
         Rule    Swiss 1941  1942  -  Oct  Mon>=1   2:00  0     -
         Rule    EU    1977  1980  -  Apr  Sun>=1   1:00u 1:00  S
         Rule    EU    1977  only  -  Sep  lastSun  1:00u 0     -
         Rule    EU    1978  only  -  Oct   1       1:00u 0     -
         Rule    EU    1979  1995  -  Sep  lastSun  1:00u 0     -
         Rule    EU    1981  max   -  Mar  lastSun  1:00u 1:00  S
         Rule    EU    1996  max   -  Oct  lastSun  1:00u 0     -

         # Zone  NAME           STDOFF      RULES  FORMAT  [UNTIL]
         Zone    Europe/Zurich  0:34:08     -      LMT     1853 Jul 16
                                0:29:45.50  -      BMT     1894 Jun
                                1:00        Swiss  CE%sT   1981
                                1:00        EU     CE%sT

         Link    Europe/Zurich  Europe/Vaduz
        ";
        let mut zic = ZicP::default();
        zic.parse(data).unwrap();
        insta::assert_debug_snapshot!(zic);
    }

    #[test]
    fn parse_zic_err() {
        // A continuation zone line where one wasn't expected.
        let data = "
         Zone    America/Menominee  -5:00   -      EST
                 -6:00              US      C%sT
        ";
        assert!(ZicP::default().parse(data).is_err());

        // No continuation line where one was expected but got Link instead.
        let data = "
         Zone    America/Menominee  -5:00   -      EST     1973 Apr 29 2:00
         Link    America/Menominee  Foo
        ";
        assert!(ZicP::default().parse(data).is_err());

        // No continuation line where one was expected but got EOF instead.
        let data = "
         Zone    America/Menominee  -5:00   -      EST     1973 Apr 29 2:00
        ";
        assert!(ZicP::default().parse(data).is_err());

        // Link conflicts with zone.
        let data = "
         Zone    America/New_York  -5:00   -      EST     1973 Apr 29 2:00
         Zone    America/Menominee  -5:00   -      EST     1973 Apr 29 2:00
         Link    America/New_York America/Menominee
        ";
        assert!(ZicP::default().parse(data).is_err());

        // Zone conflicts with link.
        let data = "
         Link    America/New_York America/Menominee
         Zone    America/New_York  -5:00   -      EST     1973 Apr 29 2:00
         Zone    America/Menominee  -5:00   -      EST     1973 Apr 29 2:00
        ";
        assert!(ZicP::default().parse(data).is_err());

        // Zone conflicts with zone.
        let data = "
         Zone    America/New_York  -5:00   -      EST     1973 Apr 29 2:00
         Zone    America/New_York  -5:00   -      EST     1973 Apr 29 2:00
        ";
        assert!(ZicP::default().parse(data).is_err());

        // Link conflicts with link.
        let data = "
         Zone    America/New_York  -5:00   -      EST     1973 Apr 29 2:00
         Zone    America/Menominee  -5:00   -      EST     1973 Apr 29 2:00
         Link    America/New_York  Foo
         Link    America/Menominee Foo
        ";
        assert!(ZicP::default().parse(data).is_err());
    }

    #[cfg(not(miri))]
    #[test]
    fn parse_rule_ok() {
        let rule: RuleP = RuleP::parse(&[
            "US", "1967", "1973", "-", "Apr", "lastSun", "2:00w", "1:00d", "D",
        ])
        .unwrap();
        insta::assert_debug_snapshot!(rule, @r###"
        RuleP {
            name: RuleNameP {
                name: "US",
            },
            from: RuleFromP {
                year: 1967,
            },
            to: Year {
                year: 1973,
            },
            inn: RuleInP {
                month: 4,
            },
            on: Last {
                weekday: Sunday,
            },
            at: RuleAtP {
                span: SpanFieldwise(
                    2h,
                ),
                suffix: Some(
                    Wall,
                ),
            },
            save: RuleSaveP {
                span: SpanFieldwise(
                    1h,
                ),
                suffix: Some(
                    Dst,
                ),
            },
            letters: RuleLettersP {
                part: "D",
            },
        }
        "###);
    }

    #[test]
    fn parse_rule_err() {
        assert!(RuleP::parse(&[
            "US", "1967", "1973", "-", "Apr", "lastSun", "2:00w", "1:00d",
            "D", "E",
        ])
        .is_err());
        assert!(RuleP::parse(&[
            "US", "1967", "1973", "-", "Apr", "lastSun", "2:00w", "1:00d",
        ])
        .is_err());
    }

    #[cfg(not(miri))]
    #[test]
    fn parse_zone_first_ok() {
        let zone: ZoneFirstP =
            ZoneFirstP::parse(&["America/Menominee", "-5:00", "-", "EST"])
                .unwrap();
        insta::assert_debug_snapshot!(zone, @r###"
        ZoneFirstP {
            name: ZoneNameP {
                name: "America/Menominee",
            },
            stdoff: ZoneStdoffP {
                span: SpanFieldwise(
                    5h ago,
                ),
            },
            rules: None,
            format: Static {
                format: "EST",
            },
            until: None,
        }
        "###);

        let zone: ZoneFirstP = ZoneFirstP::parse(&[
            "America/Menominee",
            "-5:00",
            "-",
            "EST",
            "1973",
            "Apr",
            "29",
            "2:00",
        ])
        .unwrap();
        insta::assert_debug_snapshot!(zone, @r###"
        ZoneFirstP {
            name: ZoneNameP {
                name: "America/Menominee",
            },
            stdoff: ZoneStdoffP {
                span: SpanFieldwise(
                    5h ago,
                ),
            },
            rules: None,
            format: Static {
                format: "EST",
            },
            until: Some(
                YearMonthDayTime {
                    year: 1973,
                    month: RuleInP {
                        month: 4,
                    },
                    day: Day {
                        day: 29,
                    },
                    duration: RuleAtP {
                        span: SpanFieldwise(
                            2h,
                        ),
                        suffix: None,
                    },
                },
            ),
        }
        "###);
    }

    #[test]
    fn parse_zone_first_err() {
        assert!(ZoneFirstP::parse(&[]).is_err());
        assert!(ZoneFirstP::parse(&["foo"]).is_err());
        assert!(ZoneFirstP::parse(&["foo", "-5"]).is_err());
        assert!(ZoneFirstP::parse(&["foo", "-5", "-"]).is_err());
        assert!(ZoneFirstP::parse(&[
            "foo", "-5", "-", "EST", "1973", "Apr", "29", "2:00", "w",
        ])
        .is_err());
    }

    #[cfg(not(miri))]
    #[test]
    fn parse_zone_continuation_ok() {
        let zone: ZoneContinuationP =
            ZoneContinuationP::parse(&["-5:00", "-", "EST"]).unwrap();
        insta::assert_debug_snapshot!(zone, @r###"
        ZoneContinuationP {
            stdoff: ZoneStdoffP {
                span: SpanFieldwise(
                    5h ago,
                ),
            },
            rules: None,
            format: Static {
                format: "EST",
            },
            until: None,
        }
        "###);

        let zone: ZoneContinuationP = ZoneContinuationP::parse(&[
            "-5:00", "-", "EST", "1973", "Apr", "29", "2:00",
        ])
        .unwrap();
        insta::assert_debug_snapshot!(zone, @r###"
        ZoneContinuationP {
            stdoff: ZoneStdoffP {
                span: SpanFieldwise(
                    5h ago,
                ),
            },
            rules: None,
            format: Static {
                format: "EST",
            },
            until: Some(
                YearMonthDayTime {
                    year: 1973,
                    month: RuleInP {
                        month: 4,
                    },
                    day: Day {
                        day: 29,
                    },
                    duration: RuleAtP {
                        span: SpanFieldwise(
                            2h,
                        ),
                        suffix: None,
                    },
                },
            ),
        }
        "###);
    }

    #[test]
    fn parse_zone_continuation_err() {
        assert!(ZoneContinuationP::parse(&[]).is_err());
        assert!(ZoneContinuationP::parse(&["-5"]).is_err());
        assert!(ZoneContinuationP::parse(&["-5", "-"]).is_err());
        assert!(ZoneContinuationP::parse(&[
            "-5", "-", "EST", "1973", "Apr", "29", "2:00", "w",
        ])
        .is_err());
    }

    #[test]
    fn parse_link_ok() {
        let link: LinkP = LinkP::parse(&["Greenwich", "G_M_T"]).unwrap();
        assert_eq!(
            link,
            LinkP {
                target: ZoneNameP { name: "Greenwich".to_string() },
                name: ZoneNameP { name: "G_M_T".to_string() },
            }
        );
    }

    #[test]
    fn parse_link_err() {
        assert!(LinkP::parse(&["Greenwich"]).is_err());
        assert!(LinkP::parse(&["Greenwich", "G_M_T", "foo"]).is_err());
        assert!(LinkP::parse(&["", "G_M_T"]).is_err());
        assert!(LinkP::parse(&["Greenwich", ""]).is_err());
    }

    #[test]
    fn parse_rule_name_ok() {
        let name: RuleNameP = "US".parse().unwrap();
        assert_eq!(name, RuleNameP { name: "US".to_string() });

        let name: RuleNameP = "U571".parse().unwrap();
        assert_eq!(name, RuleNameP { name: "U571".to_string() });

        let name: RuleNameP = "U+-1".parse().unwrap();
        assert_eq!(name, RuleNameP { name: "U+-1".to_string() });

        // We allow this because it's a little tricky to forbid them in
        // unquoted name fields only, and because it doesn't seem too
        // horrible to do so. Although if these characters are given special
        // significance in the future, we could wind up misinterpreting them.
        let name: RuleNameP =
            r#"U!$%&'()*,/:;<=>?@[\]^`{|}~S"#.parse().unwrap();
        assert_eq!(
            name,
            RuleNameP { name: r#"U!$%&'()*,/:;<=>?@[\]^`{|}~S"#.to_string() }
        );
    }

    #[test]
    fn parse_rule_name_err() {
        assert!("".parse::<RuleNameP>().is_err());
        assert!("+U".parse::<RuleNameP>().is_err());
        assert!("-U".parse::<RuleNameP>().is_err());
        assert!("0U".parse::<RuleNameP>().is_err());
        assert!("9U".parse::<RuleNameP>().is_err());
    }

    #[test]
    fn parse_rule_from_ok() {
        let to: RuleFromP = "2025".parse().unwrap();
        assert_eq!(to, RuleFromP { year: t::Year::new(2025).unwrap() });
        let to: RuleFromP = "9999".parse().unwrap();
        assert_eq!(to, RuleFromP { year: t::Year::new(9999).unwrap() });
        let to: RuleFromP = "-9999".parse().unwrap();
        assert_eq!(to, RuleFromP { year: t::Year::new(-9999).unwrap() });
    }

    #[test]
    fn parse_rule_from_err() {
        assert!("10000".parse::<RuleFromP>().is_err());
        assert!("-10000".parse::<RuleFromP>().is_err());
    }

    #[test]
    fn parse_rule_to_ok() {
        let to: RuleToP = "2025".parse().unwrap();
        assert_eq!(to, RuleToP::Year { year: t::Year::new(2025).unwrap() });
        let to: RuleToP = "9999".parse().unwrap();
        assert_eq!(to, RuleToP::Year { year: t::Year::new(9999).unwrap() });
        let to: RuleToP = "-9999".parse().unwrap();
        assert_eq!(to, RuleToP::Year { year: t::Year::new(-9999).unwrap() });

        let to: RuleToP = "o".parse().unwrap();
        assert_eq!(to, RuleToP::Only);
        let to: RuleToP = "only".parse().unwrap();
        assert_eq!(to, RuleToP::Only);

        let to: RuleToP = "m".parse().unwrap();
        assert_eq!(to, RuleToP::Max);
        let to: RuleToP = "max".parse().unwrap();
        assert_eq!(to, RuleToP::Max);
        let to: RuleToP = "maximum".parse().unwrap();
        assert_eq!(to, RuleToP::Max);
    }

    #[test]
    fn parse_rule_to_err() {
        assert!("10000".parse::<RuleToP>().is_err());
        assert!("-10000".parse::<RuleToP>().is_err());

        assert!("oonly".parse::<RuleToP>().is_err());
        assert!("ononly".parse::<RuleToP>().is_err());
        assert!("onlyy".parse::<RuleToP>().is_err());
        assert!("only ".parse::<RuleToP>().is_err());

        assert!("mmaximum".parse::<RuleToP>().is_err());
        assert!("mamaximum".parse::<RuleToP>().is_err());
        assert!("maxmaximum".parse::<RuleToP>().is_err());
        assert!("maximumm".parse::<RuleToP>().is_err());
        assert!("maximum ".parse::<RuleToP>().is_err());
    }

    #[test]
    fn parse_rule_in_ok() {
        let inn: RuleInP = "Ja".parse().unwrap();
        assert_eq!(inn.month.get(), 1);
        let inn: RuleInP = "January".parse().unwrap();
        assert_eq!(inn.month.get(), 1);

        let inn: RuleInP = "F".parse().unwrap();
        assert_eq!(inn.month.get(), 2);
        let inn: RuleInP = "February".parse().unwrap();
        assert_eq!(inn.month.get(), 2);

        let inn: RuleInP = "Mar".parse().unwrap();
        assert_eq!(inn.month.get(), 3);
        let inn: RuleInP = "March".parse().unwrap();
        assert_eq!(inn.month.get(), 3);

        let inn: RuleInP = "Ap".parse().unwrap();
        assert_eq!(inn.month.get(), 4);
        let inn: RuleInP = "April".parse().unwrap();
        assert_eq!(inn.month.get(), 4);

        let inn: RuleInP = "May".parse().unwrap();
        assert_eq!(inn.month.get(), 5);

        let inn: RuleInP = "Jun".parse().unwrap();
        assert_eq!(inn.month.get(), 6);
        let inn: RuleInP = "June".parse().unwrap();
        assert_eq!(inn.month.get(), 6);

        let inn: RuleInP = "Jul".parse().unwrap();
        assert_eq!(inn.month.get(), 7);
        let inn: RuleInP = "July".parse().unwrap();
        assert_eq!(inn.month.get(), 7);

        let inn: RuleInP = "Au".parse().unwrap();
        assert_eq!(inn.month.get(), 8);
        let inn: RuleInP = "August".parse().unwrap();
        assert_eq!(inn.month.get(), 8);

        let inn: RuleInP = "S".parse().unwrap();
        assert_eq!(inn.month.get(), 9);
        let inn: RuleInP = "September".parse().unwrap();
        assert_eq!(inn.month.get(), 9);

        let inn: RuleInP = "O".parse().unwrap();
        assert_eq!(inn.month.get(), 10);
        let inn: RuleInP = "October".parse().unwrap();
        assert_eq!(inn.month.get(), 10);

        let inn: RuleInP = "N".parse().unwrap();
        assert_eq!(inn.month.get(), 11);
        let inn: RuleInP = "November".parse().unwrap();
        assert_eq!(inn.month.get(), 11);

        let inn: RuleInP = "D".parse().unwrap();
        assert_eq!(inn.month.get(), 12);
        let inn: RuleInP = "December".parse().unwrap();
        assert_eq!(inn.month.get(), 12);
    }

    #[test]
    fn parse_rule_in_err() {
        assert!("J".parse::<RuleInP>().is_err());
        assert!("Januaryy".parse::<RuleInP>().is_err());
        assert!("JJanuary".parse::<RuleInP>().is_err());
        assert!("JaJanuary".parse::<RuleInP>().is_err());
    }

    #[test]
    fn parse_rule_on_ok() {
        // Specific day.
        let on: RuleOnP = "5".parse().unwrap();
        assert_eq!(on, RuleOnP::Day { day: t::Day::new(5).unwrap() });
        let on: RuleOnP = "05".parse().unwrap();
        assert_eq!(on, RuleOnP::Day { day: t::Day::new(5).unwrap() });
        let on: RuleOnP = "31".parse().unwrap();
        assert_eq!(on, RuleOnP::Day { day: t::Day::new(31).unwrap() });

        // Last weekday of month.
        let on: RuleOnP = "lastSu".parse().unwrap();
        assert_eq!(on, RuleOnP::Last { weekday: Weekday::Sunday });
        let on: RuleOnP = "lastSun".parse().unwrap();
        assert_eq!(on, RuleOnP::Last { weekday: Weekday::Sunday });
        let on: RuleOnP = "lastSund".parse().unwrap();
        assert_eq!(on, RuleOnP::Last { weekday: Weekday::Sunday });
        let on: RuleOnP = "lastSunda".parse().unwrap();
        assert_eq!(on, RuleOnP::Last { weekday: Weekday::Sunday });
        let on: RuleOnP = "lastSunday".parse().unwrap();
        assert_eq!(on, RuleOnP::Last { weekday: Weekday::Sunday });

        // Weekday on or before a day of the month.
        let on: RuleOnP = "Sun<=25".parse().unwrap();
        assert_eq!(
            on,
            RuleOnP::OnOrBefore {
                weekday: Weekday::Sunday,
                day: t::Day::new(25).unwrap()
            }
        );
        let on: RuleOnP = "Sunday<=25".parse().unwrap();
        assert_eq!(
            on,
            RuleOnP::OnOrBefore {
                weekday: Weekday::Sunday,
                day: t::Day::new(25).unwrap()
            }
        );

        // Weekday on or after a day of the month.
        let on: RuleOnP = "Sun>=8".parse().unwrap();
        assert_eq!(
            on,
            RuleOnP::OnOrAfter {
                weekday: Weekday::Sunday,
                day: t::Day::new(8).unwrap()
            }
        );
        let on: RuleOnP = "Sunday>=8".parse().unwrap();
        assert_eq!(
            on,
            RuleOnP::OnOrAfter {
                weekday: Weekday::Sunday,
                day: t::Day::new(8).unwrap()
            }
        );
    }

    #[test]
    fn parse_rule_on_err() {
        // Invalid specific day.
        assert!("0".parse::<RuleOnP>().is_err());
        assert!("00000".parse::<RuleOnP>().is_err());
        assert!("00000000000000000000000000".parse::<RuleOnP>().is_err());
        assert!("32".parse::<RuleOnP>().is_err());

        // Invalid last weekday of month.
        assert!("lastS".parse::<RuleOnP>().is_err());
        assert!("lastSSunday".parse::<RuleOnP>().is_err());
        assert!("lastSuSunday".parse::<RuleOnP>().is_err());
        assert!("lastSundayy".parse::<RuleOnP>().is_err());
        assert!("lastZ".parse::<RuleOnP>().is_err());
        assert!("last".parse::<RuleOnP>().is_err());

        // Invalid weekday on or before a day of the month.
        assert!("S<=25".parse::<RuleOnP>().is_err());
        assert!("Sun<=0".parse::<RuleOnP>().is_err());
        assert!("Sun<=32".parse::<RuleOnP>().is_err());

        // Invalid weekday on or after a day of the month.
        assert!("S>=25".parse::<RuleOnP>().is_err());
        assert!("Sun>=0".parse::<RuleOnP>().is_err());
        assert!("Sun>=32".parse::<RuleOnP>().is_err());

        // Unrecognized format.
        assert!("abc".parse::<RuleOnP>().is_err());
        assert!("Sun<25".parse::<RuleOnP>().is_err());
        assert!("Sun>25".parse::<RuleOnP>().is_err());
        assert!("Sun < 25".parse::<RuleOnP>().is_err());
        assert!("Sun > 25".parse::<RuleOnP>().is_err());
        assert!("Sun <= 25".parse::<RuleOnP>().is_err());
        assert!("Sun >= 25".parse::<RuleOnP>().is_err());
    }

    #[test]
    fn parse_rule_at_ok() {
        let at: RuleAtP = "5".parse().unwrap();
        assert_eq!(at, RuleAtP { span: td(5 * 60 * 60, 0), suffix: None });

        let at: RuleAtP = "5w".parse().unwrap();
        assert_eq!(
            at,
            RuleAtP {
                span: td(5 * 60 * 60, 0),
                suffix: Some(RuleAtSuffixP::Wall)
            }
        );

        let at: RuleAtP = "-5w".parse().unwrap();
        assert_eq!(
            at,
            RuleAtP {
                span: td(-5 * 60 * 60, 0),
                suffix: Some(RuleAtSuffixP::Wall)
            }
        );

        let at: RuleAtP = "-".parse().unwrap();
        assert_eq!(at, RuleAtP { span: td(0, 0), suffix: None });

        let at: RuleAtP = "-s".parse().unwrap();
        assert_eq!(
            at,
            RuleAtP { span: td(0, 0), suffix: Some(RuleAtSuffixP::Standard) }
        );
    }

    #[test]
    fn parse_rule_at_err() {
        assert!("".parse::<RuleAtP>().is_err());
        assert!("w".parse::<RuleAtP>().is_err());
    }

    #[test]
    fn parse_rule_at_suffix_ok() {
        let suffix: RuleAtSuffixP = "w".parse().unwrap();
        assert_eq!(suffix, RuleAtSuffixP::Wall);
        let suffix: RuleAtSuffixP = "s".parse().unwrap();
        assert_eq!(suffix, RuleAtSuffixP::Standard);
        let suffix: RuleAtSuffixP = "u".parse().unwrap();
        assert_eq!(suffix, RuleAtSuffixP::Universal);
        let suffix: RuleAtSuffixP = "g".parse().unwrap();
        assert_eq!(suffix, RuleAtSuffixP::Universal);
        let suffix: RuleAtSuffixP = "z".parse().unwrap();
        assert_eq!(suffix, RuleAtSuffixP::Universal);
    }

    #[test]
    fn parse_rule_at_suffix_err() {
        assert!("W".parse::<RuleAtSuffixP>().is_err());
        assert!("w ".parse::<RuleAtSuffixP>().is_err());
        assert!(" w".parse::<RuleAtSuffixP>().is_err());
        assert!("ww".parse::<RuleAtSuffixP>().is_err());
        assert!("".parse::<RuleAtSuffixP>().is_err());
    }

    #[test]
    fn parse_rule_save_ok() {
        let at: RuleSaveP = "5".parse().unwrap();
        assert_eq!(at, RuleSaveP { span: td(5 * 60 * 60, 0), suffix: None });

        let at: RuleSaveP = "5s".parse().unwrap();
        assert_eq!(
            at,
            RuleSaveP {
                span: td(5 * 60 * 60, 0),
                suffix: Some(RuleSaveSuffixP::Standard)
            }
        );

        let at: RuleSaveP = "-5s".parse().unwrap();
        assert_eq!(
            at,
            RuleSaveP {
                span: td(-5 * 60 * 60, 0),
                suffix: Some(RuleSaveSuffixP::Standard)
            }
        );

        let at: RuleSaveP = "-".parse().unwrap();
        assert_eq!(at, RuleSaveP { span: td(0, 0), suffix: None });

        let at: RuleSaveP = "-s".parse().unwrap();
        assert_eq!(
            at,
            RuleSaveP {
                span: td(0, 0),
                suffix: Some(RuleSaveSuffixP::Standard)
            }
        );
    }

    #[test]
    fn parse_rule_save_err() {
        assert!("".parse::<RuleSaveP>().is_err());
        assert!("s".parse::<RuleSaveP>().is_err());
    }

    #[test]
    fn parse_rule_save_suffix_ok() {
        let suffix: RuleSaveSuffixP = "s".parse().unwrap();
        assert_eq!(suffix, RuleSaveSuffixP::Standard);
        let suffix: RuleSaveSuffixP = "d".parse().unwrap();
        assert_eq!(suffix, RuleSaveSuffixP::Dst);
    }

    #[test]
    fn parse_rule_save_suffix_err() {
        assert!("S".parse::<RuleSaveSuffixP>().is_err());
        assert!("s ".parse::<RuleSaveSuffixP>().is_err());
        assert!(" s".parse::<RuleSaveSuffixP>().is_err());
        assert!("ss".parse::<RuleSaveSuffixP>().is_err());
        assert!("".parse::<RuleSaveSuffixP>().is_err());
    }

    #[test]
    fn parse_rule_letters_ok() {
        let letters: RuleLettersP = "-".parse().unwrap();
        assert_eq!(letters, RuleLettersP { part: "".to_string() });

        let letters: RuleLettersP = "S".parse().unwrap();
        assert_eq!(letters, RuleLettersP { part: "S".to_string() });

        let letters: RuleLettersP = "D".parse().unwrap();
        assert_eq!(letters, RuleLettersP { part: "D".to_string() });
    }

    #[test]
    fn parse_zone_name_ok() {
        let name: ZoneNameP = "foo".parse().unwrap();
        assert_eq!(name, ZoneNameP { name: "foo".to_string() });

        let name: ZoneNameP = "foo/.../bar".parse().unwrap();
        assert_eq!(name, ZoneNameP { name: "foo/.../bar".to_string() });

        let name: ZoneNameP = "foo bar".parse().unwrap();
        assert_eq!(name, ZoneNameP { name: "foo bar".to_string() });
    }

    #[test]
    fn parse_zone_name_err() {
        assert!("".parse::<ZoneNameP>().is_err());
        assert!("foo/./bar".parse::<ZoneNameP>().is_err());
        assert!("foo/../bar".parse::<ZoneNameP>().is_err());
    }

    #[test]
    fn parse_zone_stdoff_ok() {
        let stdoff: ZoneStdoffP = "5".parse().unwrap();
        assert_eq!(stdoff, ZoneStdoffP { span: td(5 * 60 * 60, 0) });

        let stdoff: ZoneStdoffP = "-5".parse().unwrap();
        assert_eq!(stdoff, ZoneStdoffP { span: td(-5 * 60 * 60, 0) });
    }

    #[test]
    fn parse_zone_stdoff_err() {
        assert!("".parse::<ZoneStdoffP>().is_err());
        assert!("a".parse::<ZoneStdoffP>().is_err());
        assert!("999999999999999999".parse::<ZoneStdoffP>().is_err());
    }

    #[test]
    fn parse_zone_rules_ok() {
        let rules: ZoneRulesP = "-".parse().unwrap();
        assert_eq!(rules, ZoneRulesP::None);

        let rules: ZoneRulesP = "foo".parse().unwrap();
        assert_eq!(
            rules,
            ZoneRulesP::Named(RuleNameP { name: "foo".to_string() })
        );

        let rules: ZoneRulesP = "5".parse().unwrap();
        assert_eq!(
            rules,
            ZoneRulesP::Save(RuleSaveP {
                span: td(5 * 60 * 60, 0),
                suffix: None,
            })
        );
        let rules: ZoneRulesP = "-5".parse().unwrap();
        assert_eq!(
            rules,
            ZoneRulesP::Save(RuleSaveP {
                span: td(-5 * 60 * 60, 0),
                suffix: None,
            })
        );
        let rules: ZoneRulesP = "-1d".parse().unwrap();
        assert_eq!(
            rules,
            ZoneRulesP::Save(RuleSaveP {
                span: td(-1 * 60 * 60, 0),
                suffix: Some(RuleSaveSuffixP::Dst),
            })
        );
    }

    #[test]
    fn parse_zone_rules_err() {
        assert!("".parse::<ZoneRulesP>().is_err());
        assert!("-foo".parse::<ZoneRulesP>().is_err());
        assert!("+foo".parse::<ZoneRulesP>().is_err());
        assert!("+5".parse::<ZoneRulesP>().is_err());
    }

    #[test]
    fn parse_zone_format_ok() {
        let format: ZoneFormatP = "E%sT".parse().unwrap();
        assert_eq!(
            format,
            ZoneFormatP::Variable {
                before: "E".to_string(),
                after: "T".to_string(),
            }
        );
        let format: ZoneFormatP = "AB%sXYZ".parse().unwrap();
        assert_eq!(
            format,
            ZoneFormatP::Variable {
                before: "AB".to_string(),
                after: "XYZ".to_string(),
            }
        );

        let format: ZoneFormatP = "%z".parse().unwrap();
        assert_eq!(format, ZoneFormatP::Offset,);

        let format: ZoneFormatP = "EST/EDT".parse().unwrap();
        assert_eq!(
            format,
            ZoneFormatP::Pair {
                std: "EST".to_string(),
                dst: "EDT".to_string(),
            }
        );
        let format: ZoneFormatP = "+EST/-EDT".parse().unwrap();
        assert_eq!(
            format,
            ZoneFormatP::Pair {
                std: "+EST".to_string(),
                dst: "-EDT".to_string(),
            }
        );

        let format: ZoneFormatP = "GMT".parse().unwrap();
        assert_eq!(format, ZoneFormatP::Static { format: "GMT".to_string() });
    }

    #[test]
    fn parse_zone_format_err() {
        assert!("".parse::<ZoneFormatP>().is_err());
        assert!("%s".parse::<ZoneFormatP>().is_err());
        assert!("A%s".parse::<ZoneFormatP>().is_err());
        assert!("%sZ".parse::<ZoneFormatP>().is_err());
        assert!("A/B/C".parse::<ZoneFormatP>().is_err());
        assert!("A&Z".parse::<ZoneFormatP>().is_err());
        assert!("A&B/YZ".parse::<ZoneFormatP>().is_err());
        assert!("AB/Y&Z".parse::<ZoneFormatP>().is_err());
        assert!("A&%sZ".parse::<ZoneFormatP>().is_err());
        assert!("A%s&Z".parse::<ZoneFormatP>().is_err());
        assert!("AB%zYZ".parse::<ZoneFormatP>().is_err());
    }

    #[test]
    fn parse_zone_until_ok() {
        let until = ZoneUntilP::parse(&["2025"]).unwrap();
        assert_eq!(
            until,
            ZoneUntilP::Year { year: t::Year::new(2025).unwrap() },
        );
        let until = ZoneUntilP::parse(&["9999"]).unwrap();
        assert_eq!(
            until,
            ZoneUntilP::Year { year: t::Year::new(9999).unwrap() },
        );
        let until = ZoneUntilP::parse(&["-9999"]).unwrap();
        assert_eq!(
            until,
            ZoneUntilP::Year { year: t::Year::new(-9999).unwrap() },
        );

        let until = ZoneUntilP::parse(&["2025", "Jan"]).unwrap();
        assert_eq!(
            until,
            ZoneUntilP::YearMonth {
                year: t::Year::new(2025).unwrap(),
                month: RuleInP { month: t::Month::new(1).unwrap() },
            },
        );

        let until = ZoneUntilP::parse(&["2025", "Jan", "5"]).unwrap();
        assert_eq!(
            until,
            ZoneUntilP::YearMonthDay {
                year: t::Year::new(2025).unwrap(),
                month: RuleInP { month: t::Month::new(1).unwrap() },
                day: RuleOnP::Day { day: t::Day::new(5).unwrap() },
            },
        );
        let until = ZoneUntilP::parse(&["2025", "Jan", "lastSun"]).unwrap();
        assert_eq!(
            until,
            ZoneUntilP::YearMonthDay {
                year: t::Year::new(2025).unwrap(),
                month: RuleInP { month: t::Month::new(1).unwrap() },
                day: RuleOnP::Last { weekday: Weekday::Sunday },
            },
        );

        let until =
            ZoneUntilP::parse(&["2025", "Jan", "lastSun", "-"]).unwrap();
        assert_eq!(
            until,
            ZoneUntilP::YearMonthDayTime {
                year: t::Year::new(2025).unwrap(),
                month: RuleInP { month: t::Month::new(1).unwrap() },
                day: RuleOnP::Last { weekday: Weekday::Sunday },
                duration: RuleAtP { span: td(0, 0), suffix: None },
            },
        );
        let until =
            ZoneUntilP::parse(&["2025", "Jan", "lastSun", "5"]).unwrap();
        assert_eq!(
            until,
            ZoneUntilP::YearMonthDayTime {
                year: t::Year::new(2025).unwrap(),
                month: RuleInP { month: t::Month::new(1).unwrap() },
                day: RuleOnP::Last { weekday: Weekday::Sunday },
                duration: RuleAtP { span: td(5 * 60 * 60, 0), suffix: None },
            },
        );
        let until =
            ZoneUntilP::parse(&["2025", "Jan", "lastSun", "-5"]).unwrap();
        assert_eq!(
            until,
            ZoneUntilP::YearMonthDayTime {
                year: t::Year::new(2025).unwrap(),
                month: RuleInP { month: t::Month::new(1).unwrap() },
                day: RuleOnP::Last { weekday: Weekday::Sunday },
                duration: RuleAtP { span: td(-5 * 60 * 60, 0), suffix: None },
            },
        );
        let until =
            ZoneUntilP::parse(&["2025", "Jan", "lastSun", "1:1:1.000000001"])
                .unwrap();
        assert_eq!(
            until,
            ZoneUntilP::YearMonthDayTime {
                year: t::Year::new(2025).unwrap(),
                month: RuleInP { month: t::Month::new(1).unwrap() },
                day: RuleOnP::Last { weekday: Weekday::Sunday },
                duration: RuleAtP { span: td(3661, 1), suffix: None },
            },
        );
        let until =
            ZoneUntilP::parse(&["2025", "Jan", "lastSun", "5u"]).unwrap();
        assert_eq!(
            until,
            ZoneUntilP::YearMonthDayTime {
                year: t::Year::new(2025).unwrap(),
                month: RuleInP { month: t::Month::new(1).unwrap() },
                day: RuleOnP::Last { weekday: Weekday::Sunday },
                duration: RuleAtP {
                    span: td(5 * 60 * 60, 0),
                    suffix: Some(RuleAtSuffixP::Universal),
                },
            },
        );
    }

    #[test]
    fn parse_zone_until_err() {
        assert!(ZoneUntilP::parse(&[]).is_err());

        assert!(ZoneUntilP::parse(&["10000"]).is_err());
        assert!(ZoneUntilP::parse(&["-10000"]).is_err());

        assert!(ZoneUntilP::parse(&["2025", "J"]).is_err());
        assert!(ZoneUntilP::parse(&["2025", "Z"]).is_err());

        assert!(ZoneUntilP::parse(&["2025", "Jan", "lastS"]).is_err());
        assert!(ZoneUntilP::parse(&["2025", "Jan", "0"]).is_err());
        assert!(ZoneUntilP::parse(&["2025", "Jan", "32"]).is_err());

        assert!(ZoneUntilP::parse(&["2025", "Jan", "lastSun", "w"]).is_err());
        assert!(ZoneUntilP::parse(&[
            "2025",
            "Jan",
            "lastSun",
            "1:1:1.0000000001"
        ])
        .is_err());
    }

    #[test]
    fn parse_year_ok() {
        assert_eq!(parse_year("0").unwrap().get(), 0);
        assert_eq!(parse_year("1").unwrap().get(), 1);
        assert_eq!(parse_year("-1").unwrap().get(), -1);
        assert_eq!(parse_year("2025").unwrap().get(), 2025);
        assert_eq!(parse_year("9999").unwrap().get(), 9999);
        assert_eq!(parse_year("-9999").unwrap().get(), -9999);
    }

    #[test]
    fn parse_year_err() {
        assert!(parse_year("-10000").is_err());
        assert!(parse_year("10000").is_err());
        assert!(parse_year("+2025").is_err());
        assert!(parse_year(" 2025").is_err());
        assert!(parse_year("2025 ").is_err());
        assert!(parse_year("9999999999999999999999999999").is_err());
    }

    #[test]
    fn parse_duration_ok() {
        assert_eq!(parse_span("-").unwrap(), td(0, 0));
        assert_eq!(parse_span("0").unwrap(), td(0, 0));
        assert_eq!(parse_span("-0").unwrap(), td(0, 0));

        assert_eq!(parse_span("1").unwrap(), td(3600, 0));
        assert_eq!(parse_span("1:1").unwrap(), td(3660, 0));
        assert_eq!(parse_span("1:1:1").unwrap(), td(3661, 0));
        assert_eq!(parse_span("1:1:1.1").unwrap(), td(3661, 100_000_000));
        assert_eq!(
            parse_span("1:1:1.123456789").unwrap(),
            td(3661, 123_456_789)
        );
        assert_eq!(parse_span("0:1:0").unwrap(), td(60, 0));
        assert_eq!(parse_span("0:0:1").unwrap(), td(1, 0));
        assert_eq!(parse_span("0:0:0.000000001").unwrap(), td(0, 1));
        assert_eq!(parse_span("0:0:0.000000000").unwrap(), td(0, 0));

        assert_eq!(parse_span("-1").unwrap(), td(-3600, 0));
        assert_eq!(parse_span("-1:1").unwrap(), td(-3660, 0));
        assert_eq!(parse_span("-1:1:1").unwrap(), td(-3661, 0));
        assert_eq!(parse_span("-1:1:1.1").unwrap(), td(-3661, -100_000_000));
        assert_eq!(
            parse_span("-1:1:1.123456789").unwrap(),
            td(-3661, -123_456_789)
        );
        assert_eq!(parse_span("-0:1:0").unwrap(), td(-60, 0));
        assert_eq!(parse_span("-0:0:1").unwrap(), td(-1, 0));
        assert_eq!(parse_span("-0:0:0.000000001").unwrap(), td(0, -1));
    }

    #[test]
    fn parse_duration_err() {
        assert!(parse_span("").is_err());
        assert!(parse_span(" ").is_err());
        assert!(parse_span("a").is_err());
        assert!(parse_span("999999999999999").is_err());
        assert!(parse_span("1:").is_err());
        assert!(parse_span("1:a").is_err());
        assert!(parse_span("1:60").is_err());
        assert!(parse_span("1:01:").is_err());
        assert!(parse_span("1:01:60").is_err());
        assert!(parse_span("1:01:59.").is_err());
        assert!(parse_span("1:01:59.0000000001").is_err());
        assert!(parse_span("1:01:59.0000000000").is_err());
        assert!(parse_span("1:01:59.000000001a").is_err());
        assert!(parse_span("1:01:59.000000001 ").is_err());
        assert!(parse_span("1::59").is_err());
        assert!(parse_span("1::.1").is_err());
        assert!(parse_span("::").is_err());
        // Maybe we should support this?
        assert!(parse_span("+1").is_err());

        // Tricky cases where the number of hours is at the limit, but there
        // are other units present.
        assert!(parse_span("175307616").is_ok());
        assert!(parse_span("175307617").is_err());
        assert!(parse_span("175307616:01").is_ok());
        assert!(parse_span("175307616:00:01").is_ok());
        assert!(parse_span("175307616:00:00.999999999").is_ok());
        // Same as above, but for negative hours.
        assert!(parse_span("-175307616").is_ok());
        assert!(parse_span("-175307617").is_err());
        assert!(parse_span("-175307616:01").is_ok());
        assert!(parse_span("-175307616:00:01").is_ok());
        assert!(parse_span("-175307616:00:00.999999999").is_ok());
    }

    #[test]
    fn parse_day_ok() {
        assert_eq!(parse_day("1").unwrap().get(), 1);
        assert_eq!(parse_day("2").unwrap().get(), 2);
        assert_eq!(parse_day("20").unwrap().get(), 20);
        assert_eq!(parse_day("30").unwrap().get(), 30);
        assert_eq!(parse_day("31").unwrap().get(), 31);

        assert_eq!(parse_day("01").unwrap().get(), 1);
        assert_eq!(parse_day("00000001").unwrap().get(), 1);
        assert_eq!(parse_day("0000000000000000000001").unwrap().get(), 1);
    }

    #[test]
    fn parse_day_err() {
        assert!(parse_day("").is_err());
        assert!(parse_day("0").is_err());
        assert!(parse_day("00").is_err());
        assert!(parse_day("32").is_err());
        assert!(parse_day("032").is_err());
    }

    #[test]
    fn parse_weekday_ok() {
        assert_eq!(parse_weekday("M").unwrap(), Weekday::Monday);
        assert_eq!(parse_weekday("Monday").unwrap(), Weekday::Monday);

        assert_eq!(parse_weekday("Tu").unwrap(), Weekday::Tuesday);
        assert_eq!(parse_weekday("Tuesday").unwrap(), Weekday::Tuesday);

        assert_eq!(parse_weekday("W").unwrap(), Weekday::Wednesday);
        assert_eq!(parse_weekday("Wednesday").unwrap(), Weekday::Wednesday);

        assert_eq!(parse_weekday("Th").unwrap(), Weekday::Thursday);
        assert_eq!(parse_weekday("Thursday").unwrap(), Weekday::Thursday);

        assert_eq!(parse_weekday("F").unwrap(), Weekday::Friday);
        assert_eq!(parse_weekday("Friday").unwrap(), Weekday::Friday);

        assert_eq!(parse_weekday("Sa").unwrap(), Weekday::Saturday);
        assert_eq!(parse_weekday("Saturday").unwrap(), Weekday::Saturday);

        assert_eq!(parse_weekday("Su").unwrap(), Weekday::Sunday);
        assert_eq!(parse_weekday("Sunday").unwrap(), Weekday::Sunday);
    }

    #[test]
    fn parse_weekday_err() {
        assert!(parse_weekday("S").is_err());
        assert!(parse_weekday("Sundayy").is_err());
        assert!(parse_weekday("SSunday").is_err());
        assert!(parse_weekday("SuSunday").is_err());
    }

    #[test]
    fn parse_fields_ok() {
        let mut fields: Vec<&str> = vec![];

        parse_fields("", &mut fields).unwrap();
        assert_eq!(fields, Vec::<&str>::new());

        parse_fields("# foo bar baz", &mut fields).unwrap();
        assert_eq!(fields, Vec::<&str>::new());

        parse_fields("a", &mut fields).unwrap();
        assert_eq!(fields, vec!["a"]);

        parse_fields("foo", &mut fields).unwrap();
        assert_eq!(fields, vec!["foo"]);

        parse_fields("foo#foo", &mut fields).unwrap();
        assert_eq!(fields, vec!["foo"]);

        parse_fields(r#""foo""#, &mut fields).unwrap();
        assert_eq!(fields, vec!["foo"]);

        parse_fields(r#"fo"o"#, &mut fields).unwrap();
        assert_eq!(fields, vec![r#"fo"o"#]);

        parse_fields("foo bar", &mut fields).unwrap();
        assert_eq!(fields, vec!["foo", "bar"]);

        parse_fields(" \x0C\t\x0B     foo  \t   bar   ", &mut fields).unwrap();
        assert_eq!(fields, vec!["foo", "bar"]);

        parse_fields(r#"foo "bar" baz"#, &mut fields).unwrap();
        assert_eq!(fields, vec!["foo", "bar", "baz"]);

        parse_fields(r#"foo"bar baz"quux"#, &mut fields).unwrap();
        assert_eq!(fields, vec![r#"foo"bar"#, r#"baz"quux"#]);

        // I believe this is the only way to encode an empty field. It's
        // unclear whether this is allowed or not, but it seems fine to allow
        // it.
        parse_fields(r#""""#, &mut fields).unwrap();
        assert_eq!(fields, vec![""]);
    }

    #[test]
    fn parse_fields_err() {
        let mut fields: Vec<&str> = vec![];

        assert!(parse_fields(r#""foo"bar"#, &mut fields).is_err());
        assert!(parse_fields(r#""foo"#, &mut fields).is_err());
        assert!(parse_fields("foo\0", &mut fields).is_err());

        // Test the line length check.
        let fits = "a".repeat(2047);
        assert!(parse_fields(&fits, &mut fields).is_ok());
        let toobig = "a".repeat(2048);
        assert!(parse_fields(&toobig, &mut fields).is_err());
    }
}
