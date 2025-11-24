/*!
This module provides parsing facilities for [RFC 9557] extensions to
[RFC 3339].

This only provides internal helper routines that can be used in other parsers.
Namely, RFC 9557 is just a backward compatible expansion to RFC 3339.

The parser in this module checks for full syntactic validity of the annotation
syntax defined in RFC 9557. However, Jiff doesn't make use of any of these
annotations except for time zone annotations. So for example,
`2024-05-25T13:33:00-05[America/New_York][foo=bar]` is valid, but the parser
will only expose the `America/New_York` annotation.

Note though that even for things that are ignored, validity
and criticality are still respected. So for example,
`2024-05-25T13:33:00-05[America/New_York][!foo=bar]` will fail to parse because
of the `!` indicating that consumers must take action on the annotation,
including by returning an error if it isn't supported.

[RFC 3339]: https://www.rfc-editor.org/rfc/rfc3339
[RFC 9557]: https://www.rfc-editor.org/rfc/rfc9557.html
*/

// Here's the specific part of Temporal's grammar that is implemented below
// (which should match what's in RFC 9557):
//
// TimeZoneAnnotation :::
//   [ AnnotationCriticalFlag[opt] TimeZoneIdentifier ]
//
// Annotations :::
//   Annotation Annotations[opt]
//
// AnnotationCriticalFlag :::
//   !
//
// TimeZoneIdentifier :::
//   TimeZoneUTCOffsetName
//   TimeZoneIANAName
//
// TimeZoneIANAName :::
//   TimeZoneIANANameComponent
//   TimeZoneIANAName / TimeZoneIANANameComponent
//
// TimeZoneIANANameComponent :::
//   TZLeadingChar
//   TimeZoneIANANameComponent TZChar
//
// Annotation :::
//   [ AnnotationCriticalFlag[opt] AnnotationKey = AnnotationValue ]
//
// AnnotationKey :::
//   AKeyLeadingChar
//   AnnotationKey AKeyChar
//
// AnnotationValue :::
//   AnnotationValueComponent
//   AnnotationValueComponent - AnnotationValue
//
// AnnotationValueComponent :::
//   Alpha AnnotationValueComponent[opt]
//   DecimalDigit AnnotationValueComponent[opt]
//
// AKeyLeadingChar :::
//   LowercaseAlpha
//   _
//
// AKeyChar :::
//   AKeyLeadingChar
//   DecimalDigit
//   -
//
// TZLeadingChar :::
//   Alpha
//   .
//   _
//
// TZChar :::
//   TZLeadingChar
//   DecimalDigit
//   -
//   +
//
// DecimalDigit :: one of
//   0 1 2 3 4 5 6 7 8 9
//
// Alpha ::: one of
//   A B C D E F G H I J K L M N O P Q R S T U V W X Y Z
//     a b c d e f g h i j k l m n o p q r s t u v w x y z
//
// LowercaseAlpha ::: one of
//   a b c d e f g h i j k l m n o p q r s t u v w x y z
//
// # N.B. This is handled by src/format/offset.rs, so we don't expand it here.
// TimeZoneUTCOffsetName :::
//   UTCOffsetMinutePrecision

use crate::{
    error::{err, Error},
    fmt::{
        offset::{self, ParsedOffset},
        temporal::{TimeZoneAnnotation, TimeZoneAnnotationKind},
        Parsed,
    },
    util::{escape, parse},
};

/// The result of parsing RFC 9557 annotations.
///
/// Currently, this only provides access to a parsed time zone annotation, if
/// present. While the parser does validate all other key/value annotations,
/// Jiff doesn't make use of them and thus does not expose them here. They are
/// only validated at a syntax level.
#[derive(Debug)]
pub(crate) struct ParsedAnnotations<'i> {
    /// The original input that all of the annotations were parsed from.
    ///
    /// N.B. This is currently unused, but potentially useful, so we leave it.
    #[allow(dead_code)]
    input: escape::Bytes<'i>,
    /// An optional time zone annotation that was extracted from the input.
    time_zone: Option<ParsedTimeZone<'i>>,
    // While we parse/validate them, we don't support any other annotations
    // at time of writing. Temporal supports calendar annotations, but I'm
    // not sure Jiff will ever go down that route.
}

impl<'i> ParsedAnnotations<'i> {
    /// Return an empty parsed annotations.
    pub(crate) fn none() -> ParsedAnnotations<'static> {
        ParsedAnnotations { input: escape::Bytes(&[]), time_zone: None }
    }

    /// Turns this parsed time zone into a structured time zone annotation,
    /// if an annotation was found. Otherwise, returns `Ok(None)`.
    ///
    /// This can return an error if the parsed offset could not be converted
    /// to a `crate::tz::Offset`.
    pub(crate) fn to_time_zone_annotation(
        &self,
    ) -> Result<Option<TimeZoneAnnotation<'i>>, Error> {
        let Some(ref parsed) = self.time_zone else { return Ok(None) };
        Ok(Some(parsed.to_time_zone_annotation()?))
    }
}

/// The result of parsing a time zone annotation.
#[derive(Debug)]
enum ParsedTimeZone<'i> {
    /// The name of an IANA time zone was found.
    Named {
        /// Whether the critical flag was seen.
        critical: bool,
        /// The parsed name.
        name: &'i str,
    },
    /// A specific UTC numeric offset was found.
    Offset {
        /// Whether the critical flag was seen.
        critical: bool,
        /// The parsed UTC offset.
        offset: ParsedOffset,
    },
}

impl<'i> ParsedTimeZone<'i> {
    /// Turns this parsed time zone into a structured time zone annotation.
    ///
    /// This can return an error if the parsed offset could not be converted
    /// to a `crate::tz::Offset`.
    ///
    /// This also includes a flag of whether the annotation is "critical" or
    /// not.
    pub(crate) fn to_time_zone_annotation(
        &self,
    ) -> Result<TimeZoneAnnotation<'i>, Error> {
        let (kind, critical) = match *self {
            ParsedTimeZone::Named { name, critical } => {
                let kind = TimeZoneAnnotationKind::from(name);
                (kind, critical)
            }
            ParsedTimeZone::Offset { ref offset, critical } => {
                let kind = TimeZoneAnnotationKind::Offset(offset.to_offset()?);
                (kind, critical)
            }
        };
        Ok(TimeZoneAnnotation { kind, critical })
    }
}

/// A parser for RFC 9557 annotations.
#[derive(Debug)]
pub(crate) struct Parser {
    /// There are currently no configuration options for this parser.
    _priv: (),
}

impl Parser {
    /// Create a new RFC 9557 annotation parser with the default configuration.
    pub(crate) const fn new() -> Parser {
        Parser { _priv: () }
    }

    /// Parse RFC 9557 annotations from the start of `input`.
    ///
    /// This only parses annotations when `input` starts with an `[`.
    ///
    /// Note that the result returned only provides access to the time zone
    /// annotation (if it was present). All other annotations are parsed and
    /// checked for validity, but are not accessible from `ParsedAnnotations`
    /// since Jiff does not make use of them.
    pub(crate) fn parse<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ParsedAnnotations<'i>>, Error> {
        let mkslice = parse::slicer(input);

        let Parsed { value: time_zone, mut input } =
            self.parse_time_zone_annotation(input)?;
        loop {
            // We don't actually do anything with any annotation that isn't
            // a time zone, but we do parse them to ensure validity and to
            // be able to fail when a critical flag is set. Otherwise, we know
            // we're done if parsing an annotation doesn't consume any input.
            let Parsed { value: did_consume, input: unconsumed } =
                self.parse_annotation(input)?;
            if !did_consume {
                break;
            }
            input = unconsumed;
        }

        let value = ParsedAnnotations {
            input: escape::Bytes(mkslice(input)),
            time_zone,
        };
        Ok(Parsed { value, input })
    }

    fn parse_time_zone_annotation<'i>(
        &self,
        mut input: &'i [u8],
    ) -> Result<Parsed<'i, Option<ParsedTimeZone<'i>>>, Error> {
        let unconsumed = input;
        if input.is_empty() || input[0] != b'[' {
            return Ok(Parsed { value: None, input: unconsumed });
        }
        input = &input[1..];

        let critical = input.starts_with(b"!");
        if critical {
            input = &input[1..];
        }

        // If we're starting with a `+` or `-`, then we know we MUST have a
        // time zone offset annotation. It can't be anything else since neither
        // an IANA annotation nor a generic key/value annotation can begin with
        // a `+` or a `-`.
        if input.starts_with(b"+") || input.starts_with(b"-") {
            const P: offset::Parser =
                offset::Parser::new().zulu(false).subminute(false);

            let Parsed { value: offset, input } = P.parse(input)?;
            let Parsed { input, .. } =
                self.parse_tz_annotation_close(input)?;
            let value = Some(ParsedTimeZone::Offset { critical, offset });
            return Ok(Parsed { value, input });
        }

        // At this point, we know it's impossible to see an offset. But we
        // could still see *either* an IANA time zone annotation or a more
        // generic key-value annotation. We don't know yet. In the latter case,
        // we'll eventually see an `=` sign. But since IANA time zone names
        // represent a superset of generic keys, we just parse what we can.
        // Once we stop, we can check for an `=`.
        let mkiana = parse::slicer(input);
        let Parsed { mut input, .. } =
            self.parse_tz_annotation_iana_name(input)?;
        // Now that we've parsed the first IANA name component, if this were
        // actually a generic key/value annotation, the `=` *must* appear here.
        // Otherwise, we assume we are trying to parse an IANA annotation as it
        // is the only other possibility and likely the most common case.
        if input.starts_with(b"=") {
            // Pretend like we parsed nothing and let the caller try to parse
            // a generic key/value annotation.
            return Ok(Parsed { value: None, input: unconsumed });
        }
        while input.starts_with(b"/") {
            input = &input[1..];
            let Parsed { input: unconsumed, .. } =
                self.parse_tz_annotation_iana_name(input)?;
            input = unconsumed;
        }
        // This is OK because all bytes in a IANA TZ annotation are guaranteed
        // to be ASCII, or else we wouldn't be here. If this turns out to be
        // a perf issue, we can do an unchecked conversion here. But I figured
        // it would be better to start conservative.
        let iana_name = core::str::from_utf8(mkiana(input)).expect("ASCII");
        let time_zone =
            Some(ParsedTimeZone::Named { critical, name: iana_name });
        // And finally, parse the closing bracket.
        let Parsed { input, .. } = self.parse_tz_annotation_close(input)?;
        Ok(Parsed { value: time_zone, input })
    }

    fn parse_annotation<'i>(
        &self,
        mut input: &'i [u8],
    ) -> Result<Parsed<'i, bool>, Error> {
        if input.is_empty() || input[0] != b'[' {
            return Ok(Parsed { value: false, input });
        }
        input = &input[1..];

        let critical = input.starts_with(b"!");
        if critical {
            input = &input[1..];
        }

        let Parsed { value: key, input } = self.parse_annotation_key(input)?;
        let Parsed { input, .. } = self.parse_annotation_separator(input)?;
        let Parsed { input, .. } = self.parse_annotation_values(input)?;
        let Parsed { input, .. } = self.parse_annotation_close(input)?;

        // If the critical flag is set, then we automatically return an error
        // because we don't support any non-time-zone annotations. When the
        // critical flag isn't set, we're "permissive" and just validate that
        // the syntax is correct (as we've already done at this point).
        if critical {
            return Err(err!(
                "found unsupported RFC 9557 annotation with key {key:?} \
                 with the critical flag ('!') set",
                key = escape::Bytes(key),
            ));
        }

        Ok(Parsed { value: true, input })
    }

    fn parse_tz_annotation_iana_name<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, &'i [u8]>, Error> {
        let mkname = parse::slicer(input);
        let Parsed { mut input, .. } =
            self.parse_tz_annotation_leading_char(input)?;
        loop {
            let Parsed { value: did_consume, input: unconsumed } =
                self.parse_tz_annotation_char(input);
            if !did_consume {
                break;
            }
            input = unconsumed;
        }
        Ok(Parsed { value: mkname(input), input })
    }

    fn parse_annotation_key<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, &'i [u8]>, Error> {
        let mkkey = parse::slicer(input);
        let Parsed { mut input, .. } =
            self.parse_annotation_key_leading_char(input)?;
        loop {
            let Parsed { value: did_consume, input: unconsumed } =
                self.parse_annotation_key_char(input);
            if !did_consume {
                break;
            }
            input = unconsumed;
        }
        Ok(Parsed { value: mkkey(input), input })
    }

    // N.B. If we ever actually need the values, this should probably return a
    // `Vec<&'i [u8]>`. (Well, no, because that wouldn't be good for core-only
    // configurations. So it will probably need to be something else. But,
    // probably Jiff will never care about other values.)
    fn parse_annotation_values<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ()>, Error> {
        let Parsed { mut input, .. } = self.parse_annotation_value(input)?;
        while input.starts_with(b"-") {
            input = &input[1..];
            let Parsed { input: unconsumed, .. } =
                self.parse_annotation_value(input)?;
            input = unconsumed;
        }
        Ok(Parsed { value: (), input })
    }

    fn parse_annotation_value<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, &'i [u8]>, Error> {
        let mkvalue = parse::slicer(input);
        let Parsed { mut input, .. } =
            self.parse_annotation_value_leading_char(input)?;
        loop {
            let Parsed { value: did_consume, input: unconsumed } =
                self.parse_annotation_value_char(input);
            if !did_consume {
                break;
            }
            input = unconsumed;
        }
        let value = mkvalue(input);
        Ok(Parsed { value, input })
    }

    fn parse_tz_annotation_leading_char<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ()>, Error> {
        if input.is_empty() {
            return Err(err!(
                "expected the start of an RFC 9557 annotation or IANA \
                 time zone component name, but found end of input instead",
            ));
        }
        if !matches!(input[0], b'_' | b'.' | b'A'..=b'Z' | b'a'..=b'z') {
            return Err(err!(
                "expected ASCII alphabetic byte (or underscore or period) \
                 at the start of an RFC 9557 annotation or time zone \
                 component name, but found {:?} instead",
                escape::Byte(input[0]),
            ));
        }
        Ok(Parsed { value: (), input: &input[1..] })
    }

    fn parse_tz_annotation_char<'i>(
        &self,
        input: &'i [u8],
    ) -> Parsed<'i, bool> {
        let is_tz_annotation_char = |byte| {
            matches!(
                byte,
                b'_' | b'.' | b'+' | b'-' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z',
            )
        };
        if input.is_empty() || !is_tz_annotation_char(input[0]) {
            return Parsed { value: false, input };
        }
        Parsed { value: true, input: &input[1..] }
    }

    fn parse_annotation_key_leading_char<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ()>, Error> {
        if input.is_empty() {
            return Err(err!(
                "expected the start of an RFC 9557 annotation key, \
                 but found end of input instead",
            ));
        }
        if !matches!(input[0], b'_' | b'a'..=b'z') {
            return Err(err!(
                "expected lowercase alphabetic byte (or underscore) \
                 at the start of an RFC 9557 annotation key, \
                 but found {:?} instead",
                escape::Byte(input[0]),
            ));
        }
        Ok(Parsed { value: (), input: &input[1..] })
    }

    fn parse_annotation_key_char<'i>(
        &self,
        input: &'i [u8],
    ) -> Parsed<'i, bool> {
        let is_annotation_key_char =
            |byte| matches!(byte, b'_' | b'-' | b'0'..=b'9' | b'a'..=b'z');
        if input.is_empty() || !is_annotation_key_char(input[0]) {
            return Parsed { value: false, input };
        }
        Parsed { value: true, input: &input[1..] }
    }

    fn parse_annotation_value_leading_char<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ()>, Error> {
        if input.is_empty() {
            return Err(err!(
                "expected the start of an RFC 9557 annotation value, \
                 but found end of input instead",
            ));
        }
        if !matches!(input[0], b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z') {
            return Err(err!(
                "expected alphanumeric ASCII byte \
                 at the start of an RFC 9557 annotation value, \
                 but found {:?} instead",
                escape::Byte(input[0]),
            ));
        }
        Ok(Parsed { value: (), input: &input[1..] })
    }

    fn parse_annotation_value_char<'i>(
        &self,
        input: &'i [u8],
    ) -> Parsed<'i, bool> {
        let is_annotation_value_char =
            |byte| matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z');
        if input.is_empty() || !is_annotation_value_char(input[0]) {
            return Parsed { value: false, input };
        }
        Parsed { value: true, input: &input[1..] }
    }

    fn parse_annotation_separator<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ()>, Error> {
        if input.is_empty() {
            return Err(err!(
                "expected an '=' after parsing an RFC 9557 annotation key, \
                 but found end of input instead",
            ));
        }
        if input[0] != b'=' {
            // If we see a /, then it's likely the user was trying to insert a
            // time zone annotation in the wrong place.
            return Err(if input[0] == b'/' {
                err!(
                    "expected an '=' after parsing an RFC 9557 annotation \
                     key, but found / instead (time zone annotations must \
                     come first)",
                )
            } else {
                err!(
                    "expected an '=' after parsing an RFC 9557 annotation \
                     key, but found {:?} instead",
                    escape::Byte(input[0]),
                )
            });
        }
        Ok(Parsed { value: (), input: &input[1..] })
    }

    fn parse_annotation_close<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ()>, Error> {
        if input.is_empty() {
            return Err(err!(
                "expected an ']' after parsing an RFC 9557 annotation key \
                 and value, but found end of input instead",
            ));
        }
        if input[0] != b']' {
            return Err(err!(
                "expected an ']' after parsing an RFC 9557 annotation key \
                 and value, but found {:?} instead",
                escape::Byte(input[0]),
            ));
        }
        Ok(Parsed { value: (), input: &input[1..] })
    }

    fn parse_tz_annotation_close<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ()>, Error> {
        if input.is_empty() {
            return Err(err!(
                "expected an ']' after parsing an RFC 9557 time zone \
                 annotation, but found end of input instead",
            ));
        }
        if input[0] != b']' {
            return Err(err!(
                "expected an ']' after parsing an RFC 9557 time zone \
                 annotation, but found {:?} instead",
                escape::Byte(input[0]),
            ));
        }
        Ok(Parsed { value: (), input: &input[1..] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_time_zone() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let p = |input| {
            Parser::new()
                .parse(input)
                .unwrap()
                .value
                .to_time_zone_annotation()
                .unwrap()
                .map(|ann| (ann.to_time_zone().unwrap(), ann.is_critical()))
        };

        insta::assert_debug_snapshot!(p(b"[America/New_York]"), @r###"
        Some(
            (
                TimeZone(
                    TZif(
                        "America/New_York",
                    ),
                ),
                false,
            ),
        )
        "###);
        insta::assert_debug_snapshot!(p(b"[!America/New_York]"), @r###"
        Some(
            (
                TimeZone(
                    TZif(
                        "America/New_York",
                    ),
                ),
                true,
            ),
        )
        "###);
        insta::assert_debug_snapshot!(p(b"[america/new_york]"), @r###"
        Some(
            (
                TimeZone(
                    TZif(
                        "America/New_York",
                    ),
                ),
                false,
            ),
        )
        "###);
        insta::assert_debug_snapshot!(p(b"[+25:59]"), @r###"
        Some(
            (
                TimeZone(
                    25:59:00,
                ),
                false,
            ),
        )
        "###);
        insta::assert_debug_snapshot!(p(b"[-25:59]"), @r###"
        Some(
            (
                TimeZone(
                    -25:59:00,
                ),
                false,
            ),
        )
        "###);
    }

    #[test]
    fn ok_empty() {
        let p = |input| Parser::new().parse(input).unwrap();

        insta::assert_debug_snapshot!(p(b""), @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "",
                time_zone: None,
            },
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"blah"), @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "",
                time_zone: None,
            },
            input: "blah",
        }
        "###);
    }

    #[test]
    fn ok_unsupported() {
        let p = |input| Parser::new().parse(input).unwrap();

        insta::assert_debug_snapshot!(
            p(b"[u-ca=chinese]"),
            @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[u-ca=chinese]",
                time_zone: None,
            },
            input: "",
        }
        "###,
        );
        insta::assert_debug_snapshot!(
            p(b"[u-ca=chinese-japanese]"),
            @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[u-ca=chinese-japanese]",
                time_zone: None,
            },
            input: "",
        }
        "###,
        );
        insta::assert_debug_snapshot!(
            p(b"[u-ca=chinese-japanese-russian]"),
            @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[u-ca=chinese-japanese-russian]",
                time_zone: None,
            },
            input: "",
        }
        "###,
        );
    }

    #[test]
    fn ok_iana() {
        let p = |input| Parser::new().parse(input).unwrap();

        insta::assert_debug_snapshot!(p(b"[America/New_York]"), @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[America/New_York]",
                time_zone: Some(
                    Named {
                        critical: false,
                        name: "America/New_York",
                    },
                ),
            },
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"[!America/New_York]"), @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[!America/New_York]",
                time_zone: Some(
                    Named {
                        critical: true,
                        name: "America/New_York",
                    },
                ),
            },
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"[UTC]"), @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[UTC]",
                time_zone: Some(
                    Named {
                        critical: false,
                        name: "UTC",
                    },
                ),
            },
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"[.._foo_../.0+-]"), @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[.._foo_../.0+-]",
                time_zone: Some(
                    Named {
                        critical: false,
                        name: ".._foo_../.0+-",
                    },
                ),
            },
            input: "",
        }
        "###);
    }

    #[test]
    fn ok_offset() {
        let p = |input| Parser::new().parse(input).unwrap();

        insta::assert_debug_snapshot!(p(b"[-00]"), @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[-00]",
                time_zone: Some(
                    Offset {
                        critical: false,
                        offset: ParsedOffset {
                            kind: Numeric(
                                -00,
                            ),
                        },
                    },
                ),
            },
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"[+00]"), @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[+00]",
                time_zone: Some(
                    Offset {
                        critical: false,
                        offset: ParsedOffset {
                            kind: Numeric(
                                +00,
                            ),
                        },
                    },
                ),
            },
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"[-05]"), @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[-05]",
                time_zone: Some(
                    Offset {
                        critical: false,
                        offset: ParsedOffset {
                            kind: Numeric(
                                -05,
                            ),
                        },
                    },
                ),
            },
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"[!+05:12]"), @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[!+05:12]",
                time_zone: Some(
                    Offset {
                        critical: true,
                        offset: ParsedOffset {
                            kind: Numeric(
                                +05:12,
                            ),
                        },
                    },
                ),
            },
            input: "",
        }
        "###);
    }

    #[test]
    fn ok_iana_unsupported() {
        let p = |input| Parser::new().parse(input).unwrap();

        insta::assert_debug_snapshot!(
            p(b"[America/New_York][u-ca=chinese-japanese-russian]"),
            @r###"
        Parsed {
            value: ParsedAnnotations {
                input: "[America/New_York][u-ca=chinese-japanese-russian]",
                time_zone: Some(
                    Named {
                        critical: false,
                        name: "America/New_York",
                    },
                ),
            },
            input: "",
        }
        "###,
        );
    }

    #[test]
    fn err_iana() {
        insta::assert_snapshot!(
            Parser::new().parse(b"[0/Foo]").unwrap_err(),
            @r###"expected ASCII alphabetic byte (or underscore or period) at the start of an RFC 9557 annotation or time zone component name, but found "0" instead"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[Foo/0Bar]").unwrap_err(),
            @r###"expected ASCII alphabetic byte (or underscore or period) at the start of an RFC 9557 annotation or time zone component name, but found "0" instead"###,
        );
    }

    #[test]
    fn err_offset() {
        insta::assert_snapshot!(
            Parser::new().parse(b"[+").unwrap_err(),
            @r###"failed to parse hours in UTC numeric offset "+": expected two digit hour after sign, but found end of input"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[+26]").unwrap_err(),
            @r###"failed to parse hours in UTC numeric offset "+26]": offset hours are not valid: parameter 'hours' with value 26 is not in the required range of 0..=25"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[-26]").unwrap_err(),
            @r###"failed to parse hours in UTC numeric offset "-26]": offset hours are not valid: parameter 'hours' with value 26 is not in the required range of 0..=25"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[+05:12:34]").unwrap_err(),
            @r###"subminute precision for UTC numeric offset "+05:12:34]" is not enabled in this context (must provide only integral minutes)"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[+05:12:34.123456789]").unwrap_err(),
            @r###"subminute precision for UTC numeric offset "+05:12:34.123456789]" is not enabled in this context (must provide only integral minutes)"###,
        );
    }

    #[test]
    fn err_critical_unsupported() {
        insta::assert_snapshot!(
            Parser::new().parse(b"[!u-ca=chinese]").unwrap_err(),
            @r###"found unsupported RFC 9557 annotation with key "u-ca" with the critical flag ('!') set"###,
        );
    }

    #[test]
    fn err_key_leading_char() {
        insta::assert_snapshot!(
            Parser::new().parse(b"[").unwrap_err(),
            @"expected the start of an RFC 9557 annotation or IANA time zone component name, but found end of input instead",
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[&").unwrap_err(),
            @r###"expected ASCII alphabetic byte (or underscore or period) at the start of an RFC 9557 annotation or time zone component name, but found "&" instead"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[Foo][").unwrap_err(),
            @"expected the start of an RFC 9557 annotation key, but found end of input instead",
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[Foo][&").unwrap_err(),
            @r###"expected lowercase alphabetic byte (or underscore) at the start of an RFC 9557 annotation key, but found "&" instead"###,
        );
    }

    #[test]
    fn err_separator() {
        insta::assert_snapshot!(
            Parser::new().parse(b"[abc").unwrap_err(),
            @"expected an ']' after parsing an RFC 9557 time zone annotation, but found end of input instead",
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[_abc").unwrap_err(),
            @"expected an ']' after parsing an RFC 9557 time zone annotation, but found end of input instead",
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[abc^").unwrap_err(),
            @r###"expected an ']' after parsing an RFC 9557 time zone annotation, but found "^" instead"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[Foo][abc").unwrap_err(),
            @"expected an '=' after parsing an RFC 9557 annotation key, but found end of input instead",
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[Foo][_abc").unwrap_err(),
            @"expected an '=' after parsing an RFC 9557 annotation key, but found end of input instead",
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[Foo][abc^").unwrap_err(),
            @r###"expected an '=' after parsing an RFC 9557 annotation key, but found "^" instead"###,
        );
    }

    #[test]
    fn err_value() {
        insta::assert_snapshot!(
            Parser::new().parse(b"[abc=").unwrap_err(),
            @"expected the start of an RFC 9557 annotation value, but found end of input instead",
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[_abc=").unwrap_err(),
            @"expected the start of an RFC 9557 annotation value, but found end of input instead",
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[abc=^").unwrap_err(),
            @r###"expected alphanumeric ASCII byte at the start of an RFC 9557 annotation value, but found "^" instead"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[abc=]").unwrap_err(),
            @r###"expected alphanumeric ASCII byte at the start of an RFC 9557 annotation value, but found "]" instead"###,
        );
    }

    #[test]
    fn err_close() {
        insta::assert_snapshot!(
            Parser::new().parse(b"[abc=123").unwrap_err(),
            @"expected an ']' after parsing an RFC 9557 annotation key and value, but found end of input instead",
        );
        insta::assert_snapshot!(
            Parser::new().parse(b"[abc=123*").unwrap_err(),
            @r###"expected an ']' after parsing an RFC 9557 annotation key and value, but found "*" instead"###,
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn err_time_zone_db_lookup() {
        // The error message snapshotted below can vary based on tzdb
        // config, so only run this when we know we've got a real tzdb.
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let p = |input| {
            Parser::new()
                .parse(input)
                .unwrap()
                .value
                .to_time_zone_annotation()
                .unwrap()
                .unwrap()
                .to_time_zone()
                .unwrap_err()
        };

        insta::assert_snapshot!(
            p(b"[Foo]"),
            @"failed to find time zone `Foo` in time zone database",
        );
    }

    #[test]
    fn err_repeated_time_zone() {
        let p = |input| Parser::new().parse(input).unwrap_err();
        insta::assert_snapshot!(
            p(b"[america/new_york][america/new_york]"),
            @"expected an '=' after parsing an RFC 9557 annotation key, but found / instead (time zone annotations must come first)",
        );
    }
}
