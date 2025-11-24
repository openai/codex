// Copyright The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This module provides the lower-level API for UTS 46.
//!
//! [`Uts46::process`] is the core that the other convenience
//! methods build on.
//!
//! UTS 46 flags map to this API as follows:
//!
//! * _CheckHyphens_ - _true_: [`Hyphens::Check`], _false_: [`Hyphens::Allow`]; the WHATWG URL Standard sets this to _false_ for normal (non-conformance-checker) user agents.
//! * _CheckBidi_ - Always _true_; cannot be configured, since this flag is _true_ even when WHATWG URL Standard _beStrict_ is _false_.
//! * _CheckJoiners_ - Always _true_; cannot be configured, since this flag is _true_ even when WHATWG URL Standard _beStrict_ is _false_.
//! * _UseSTD3ASCIIRules_ - _true_: [`AsciiDenyList::STD3`], _false_: [`AsciiDenyList::EMPTY`]; however, the check the WHATWG URL Standard performs right after the UTS 46 invocation corresponds to [`AsciiDenyList::URL`].
//! * _Transitional_Processing_ - Always _false_ but could be implemented as a preprocessing step. This flag is deprecated and for Web purposes the transition is over in the sense that all of Firefox, Safari, or Chrome set this flag to _false_.
//! * _VerifyDnsLength_ - _true_: [`DnsLength::Verify`], _false_: [`DnsLength::Ignore`]; the WHATWG URL Standard sets this to _false_ for normal (non-conformance-checker) user agents.
//! * _IgnoreInvalidPunycode_ - Always _false_; cannot be configured. (Not yet covered by the WHATWG URL Standard, but 2 out of 3 major browser clearly behave as if this was _false_).

use crate::punycode::Decoder;
use crate::punycode::InternalCaller;
use alloc::borrow::Cow;
use alloc::string::String;
use core::fmt::Write;
use idna_adapter::*;
use smallvec::SmallVec;
use utf8_iter::Utf8CharsEx;

/// ICU4C-compatible constraint.
/// https://unicode-org.atlassian.net/browse/ICU-13727
const PUNYCODE_DECODE_MAX_INPUT_LENGTH: usize = 2000;

/// ICU4C-compatible constraint. (Note: ICU4C measures
/// UTF-16 and we measure UTF-32. This means that we
/// allow longer non-BMP inputs. For this implementation,
/// the denial-of-service scaling does not depend on BMP vs.
/// non-BMP: only the scalar values matter.)
///
/// https://unicode-org.atlassian.net/browse/ICU-13727
const PUNYCODE_ENCODE_MAX_INPUT_LENGTH: usize = 1000;

/// For keeping track of what kind of numerals have been
/// seen in an RTL label.
#[derive(Debug, PartialEq, Eq)]
enum RtlNumeralState {
    Undecided,
    European,
    Arabic,
}

/// Computes the mask for upper-case ASCII.
const fn upper_case_mask() -> u128 {
    let mut accu = 0u128;
    let mut b = 0u8;
    while b < 128 {
        if (b >= b'A') && (b <= b'Z') {
            accu |= 1u128 << b;
        }
        b += 1;
    }
    accu
}

/// Bit set for upper-case ASCII.
const UPPER_CASE_MASK: u128 = upper_case_mask();

/// Computes the mask for glyphless ASCII.
const fn glyphless_mask() -> u128 {
    let mut accu = 0u128;
    let mut b = 0u8;
    while b < 128 {
        if (b <= b' ') || (b == 0x7F) {
            accu |= 1u128 << b;
        }
        b += 1;
    }
    accu
}

/// Bit set for glyphless ASCII.
const GLYPHLESS_MASK: u128 = glyphless_mask();

/// The mask for the ASCII dot.
const DOT_MASK: u128 = 1 << b'.';

/// Computes the ASCII deny list for STD3 ASCII rules.
const fn ldh_mask() -> u128 {
    let mut accu = 0u128;
    let mut b = 0u8;
    while b < 128 {
        if !((b >= b'a' && b <= b'z') || (b >= b'0' && b <= b'9') || b == b'-' || b == b'.') {
            accu |= 1u128 << b;
        }
        b += 1;
    }
    accu
}

const PUNYCODE_PREFIX: u32 =
    ((b'-' as u32) << 24) | ((b'-' as u32) << 16) | ((b'N' as u32) << 8) | b'X' as u32;

const PUNYCODE_PREFIX_MASK: u32 = (0xFF << 24) | (0xFF << 16) | (0xDF << 8) | 0xDF;

fn write_punycode_label<W: Write + ?Sized>(
    label: &[char],
    sink: &mut W,
) -> Result<(), ProcessingError> {
    sink.write_str("xn--")?;
    crate::punycode::encode_into::<_, _, InternalCaller>(label.iter().copied(), sink)?;
    Ok(())
}

#[inline(always)]
fn has_punycode_prefix(slice: &[u8]) -> bool {
    if slice.len() < 4 {
        return false;
    }
    // Sadly, the optimizer doesn't figure out that more idiomatic code
    // should compile to masking on 32-bit value.
    let a = slice[0];
    let b = slice[1];
    let c = slice[2];
    let d = slice[3];
    let u = (u32::from(d) << 24) | (u32::from(c) << 16) | (u32::from(b) << 8) | u32::from(a);
    (u & PUNYCODE_PREFIX_MASK) == PUNYCODE_PREFIX
}

#[inline(always)]
fn in_inclusive_range8(u: u8, start: u8, end: u8) -> bool {
    u.wrapping_sub(start) <= (end - start)
}

#[inline(always)]
fn in_inclusive_range_char(c: char, start: char, end: char) -> bool {
    u32::from(c).wrapping_sub(u32::from(start)) <= (u32::from(end) - u32::from(start))
}

#[inline(always)]
fn is_passthrough_ascii_label(label: &[u8]) -> bool {
    // XXX if we aren't performing _CheckHyphens_, this could
    // check for "xn--" and pass through YouTube CDN node names.
    if label.len() >= 4 && label[2] == b'-' && label[3] == b'-' {
        return false;
    }
    if let Some((&first, tail)) = label.split_first() {
        // We need to check the first and last character
        // more strictly in case this turns out to be a
        // label in a bidi domain name. This has the side
        // effect that this function only accepts labels
        // that also conform to the STD3 rules.
        //
        // XXX: If we are in the fail-fast mode (i.e. we don't need
        // to be able to overwrite anything with U+FFFD), we could
        // merely record that we've seen a digit here and error out
        // if we later discover that the domain name is a bidi
        // domain name.
        if !in_inclusive_range8(first, b'a', b'z') {
            return false;
        }
        for &b in tail {
            // If we used LDH_MASK, we'd have to check
            // the bytes for the ASCII range anyhow.
            if in_inclusive_range8(b, b'a', b'z') {
                continue;
            }
            if in_inclusive_range8(b, b'0', b'9') {
                continue;
            }
            if b == b'-' {
                continue;
            }
            return false;
        }
        label.last() != Some(&b'-')
    } else {
        // empty
        true
    }
}

#[inline(always)]
fn split_ascii_fast_path_prefix(label: &[u8]) -> (&[u8], &[u8]) {
    if let Some(pos) = label.iter().position(|b| !b.is_ascii()) {
        if pos == 0 {
            // First is non-ASCII
            (&[], label)
        } else {
            // Leave one ASCII character in the suffix
            // in case it's a letter that a combining
            // character combines with.
            let (head, tail) = label.split_at(pos - 1);
            (head, tail)
        }
    } else {
        // All ASCII
        (label, &[])
    }
}

// Input known to be lower-case, but may contain non-ASCII.
#[inline(always)]
fn apply_ascii_deny_list_to_lower_cased_unicode(c: char, deny_list: u128) -> char {
    if let Some(shifted) = 1u128.checked_shl(u32::from(c)) {
        if (deny_list & shifted) == 0 {
            c
        } else {
            '\u{FFFD}'
        }
    } else {
        c
    }
}

// Input known to be ASCII, but may contain upper case ASCII.
#[inline(always)]
fn apply_ascii_deny_list_to_potentially_upper_case_ascii(b: u8, deny_list: u128) -> char {
    if (deny_list & (1u128 << b)) == 0 {
        return char::from(b);
    }
    if in_inclusive_range8(b, b'A', b'Z') {
        return char::from(b + 0x20);
    }
    '\u{FFFD}'
}

#[inline(always)]
fn is_ascii(label: &[char]) -> bool {
    for c in label.iter() {
        if !c.is_ascii() {
            return false;
        }
    }
    true
}

#[derive(PartialEq, Eq, Copy, Clone)]
enum PunycodeClassification {
    Ascii,
    Unicode,
    Error,
}

#[inline(always)]
fn classify_for_punycode(label: &[char]) -> PunycodeClassification {
    let mut iter = label.iter().copied();
    loop {
        if let Some(c) = iter.next() {
            if c.is_ascii() {
                continue;
            }
            if c == '\u{FFFD}' {
                return PunycodeClassification::Error;
            }
            for c in iter {
                if c == '\u{FFFD}' {
                    return PunycodeClassification::Error;
                }
            }
            return PunycodeClassification::Unicode;
        }
        return PunycodeClassification::Ascii;
    }
}

/// The ASCII deny list to be applied.
#[derive(PartialEq, Eq, Copy, Clone)]
#[repr(transparent)]
pub struct AsciiDenyList {
    bits: u128,
}

impl AsciiDenyList {
    /// Computes (preferably at compile time) an ASCII deny list.
    ///
    /// Setting `deny_glyphless` to `true` denies U+0020 SPACE and below
    /// as well as U+007F DELETE for convenience without having to list
    /// these characters in the `deny_list` string.
    ///
    /// `deny_list` is the list of ASCII characters to deny. This
    /// list must not contain any of:
    /// * Letters
    /// * Digits
    /// * Hyphen
    /// * Dot (period / full-stop)
    /// * Non-ASCII
    ///
    /// # Panics
    ///
    /// If the deny list contains characters listed as prohibited above.
    pub const fn new(deny_glyphless: bool, deny_list: &str) -> Self {
        let mut bits = UPPER_CASE_MASK;
        if deny_glyphless {
            bits |= GLYPHLESS_MASK;
        }
        let mut i = 0;
        let bytes = deny_list.as_bytes();
        while i < bytes.len() {
            let b = bytes[i];
            assert!(b < 0x80, "ASCII deny list must be ASCII.");
            // assert_ne not yet available in const context.
            assert!(b != b'.', "ASCII deny list must not contain the dot.");
            assert!(b != b'-', "ASCII deny list must not contain the hyphen.");
            assert!(
                !((b >= b'0') && (b <= b'9')),
                "ASCII deny list must not contain digits."
            );
            assert!(
                !((b >= b'a') && (b <= b'z')),
                "ASCII deny list must not contain letters."
            );
            assert!(
                !((b >= b'A') && (b <= b'Z')),
                "ASCII deny list must not contain letters."
            );
            bits |= 1u128 << b;
            i += 1;
        }
        AsciiDenyList { bits }
    }

    /// No ASCII deny list. This corresponds to _UseSTD3ASCIIRules=false_.
    ///
    /// Equivalent to `AsciiDenyList::new(false, "")`.
    ///
    /// Note: Not denying the space and control characters can result in
    /// strange behavior. Without a deny list provided to the UTS 46
    /// operation, the caller is expected perform filtering afterwards,
    /// but it's more efficient to use `AsciiDenyList` than post-processing,
    /// because the internals of this crate can optimize away checks in
    /// certain cases.
    pub const EMPTY: AsciiDenyList = AsciiDenyList::new(false, "");

    /// The STD3 deny list. This corresponds to _UseSTD3ASCIIRules=true_.
    ///
    /// Note that this deny list rejects the underscore, which occurs in
    /// pseudo-hosts used by various TXT record-based protocols, and also
    /// characters that may occurs in non-DNS naming, such as NetBIOS.
    pub const STD3: AsciiDenyList = AsciiDenyList { bits: ldh_mask() };

    /// [Forbidden domain code point](https://url.spec.whatwg.org/#forbidden-domain-code-point) from the WHATWG URL Standard.
    ///
    /// Equivalent to `AsciiDenyList::new(true, "%#/:<>?@[\\]^|")`.
    ///
    /// Note that this deny list rejects IPv6 addresses, so (as in URL
    /// parsing) you need to check for IPv6 addresses first and not
    /// put them through UTS 46 processing.
    pub const URL: AsciiDenyList = AsciiDenyList::new(true, "%#/:<>?@[\\]^|");
}

/// The _CheckHyphens_ mode.
#[derive(PartialEq, Eq, Copy, Clone)]
#[non_exhaustive] // non_exhaustive in case a middle mode that prohibits only first and last position needs to be added
pub enum Hyphens {
    /// _CheckHyphens=false_: Do not place positional restrictions on hyphens.
    ///
    /// This mode is used by the WHATWG URL Standard for normal User Agent processing
    /// (i.e. not conformance checking).
    Allow,

    /// Prohibit hyphens in the first and last position in the label but allow in
    /// the third and fourth position.
    ///
    /// Note that this mode rejects real-world names, including some GitHub user pages.
    CheckFirstLast,

    /// _CheckHyphens=true_: Prohibit hyphens in the first, third, fourth,
    /// and last position in the label.
    ///
    /// Note that this mode rejects real-world names, including YouTube CDN nodes
    /// and some GitHub user pages.
    Check,
}

/// The UTS 46 _VerifyDNSLength_ flag.
#[derive(PartialEq, Eq, Copy, Clone)]
#[non_exhaustive]
pub enum DnsLength {
    /// _VerifyDNSLength=false_. (Possibly relevant for allowing non-DNS naming systems.)
    Ignore,
    /// _VerifyDNSLength=true_ with the exception that the trailing root label dot is
    /// allowed.
    VerifyAllowRootDot,
    /// _VerifyDNSLength=true_. (The trailing root label dot is not allowed.)
    Verify,
}

/// Policy for customizing behavior in case of an error.
#[derive(PartialEq, Eq, Copy, Clone)]
#[non_exhaustive]
pub enum ErrorPolicy {
    /// Return as early as possible without producing output in case of error.
    FailFast,
    /// In case of error, mark errors with the REPLACEMENT CHARACTER. (The output
    /// containing REPLACEMENT CHARACTERs may be show to the user to illustrate
    /// what was wrong but must not be used for naming in a network protocol.)
    MarkErrors,
}

/// The success outcome of [`Uts46::process`]
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum ProcessingSuccess {
    /// There were no errors. The caller must consider the input to be the output.
    ///
    /// This asserts that the input can be safely passed to [`core::str::from_utf8_unchecked`].
    ///
    /// (Distinct from `WroteToSink` in order to allow `Cow` behavior to be implemented on top of
    /// [`Uts46::process`].)
    Passthrough,

    /// There were no errors. The caller must consider what was written to the sink to be the output.
    ///
    /// (Distinct from `Passthrough` in order to allow `Cow` behavior to be implemented on top of
    /// [`Uts46::process`].)
    WroteToSink,
}

/// The failure outcome of [`Uts46::process`]
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum ProcessingError {
    /// There was a validity error according to the chosen options.
    ///
    /// In case of `Operation::ToAscii`, there is no output. Otherwise, output was written to the
    /// sink and the output contains at least one U+FFFD REPLACEMENT CHARACTER to denote an error.
    ValidityError,

    /// The sink emitted [`core::fmt::Error`]. The partial output written to the sink must not
    /// be used.
    SinkError,
}

impl From<core::fmt::Error> for ProcessingError {
    fn from(_: core::fmt::Error) -> Self {
        ProcessingError::SinkError
    }
}

impl From<crate::punycode::PunycodeEncodeError> for ProcessingError {
    fn from(_: crate::punycode::PunycodeEncodeError) -> Self {
        unreachable!(
            "Punycode overflows should not be possible due to PUNYCODE_ENCODE_MAX_INPUT_LENGTH"
        );
    }
}

#[derive(Debug, Clone, Copy)]
enum AlreadyAsciiLabel<'a> {
    MixedCaseAscii(&'a [u8]),
    MixedCasePunycode(&'a [u8]),
    Other,
}

/// Performs the _VerifyDNSLength_ check on the output of the _ToASCII_ operation.
///
/// If the second argument is `false`, the trailing root label dot is allowed.
///
/// # Panics
///
/// Panics in debug mode if the argument isn't ASCII.
pub fn verify_dns_length(domain_name: &str, allow_trailing_dot: bool) -> bool {
    let bytes = domain_name.as_bytes();
    debug_assert!(bytes.is_ascii());
    let domain_name_without_trailing_dot = if let Some(without) = bytes.strip_suffix(b".") {
        if !allow_trailing_dot {
            return false;
        }
        without
    } else {
        bytes
    };
    if domain_name_without_trailing_dot.len() > 253 {
        return false;
    }
    for label in domain_name_without_trailing_dot.split(|b| *b == b'.') {
        if label.is_empty() {
            return false;
        }
        if label.len() > 63 {
            return false;
        }
    }
    true
}

/// An implementation of UTS #46.
pub struct Uts46 {
    data: idna_adapter::Adapter,
}

#[cfg(feature = "compiled_data")]
impl Default for Uts46 {
    fn default() -> Self {
        Self::new()
    }
}

impl Uts46 {
    /// Constructor using data compiled into the binary.
    #[cfg(feature = "compiled_data")]
    pub const fn new() -> Self {
        Self {
            data: idna_adapter::Adapter::new(),
        }
    }

    // XXX Should there be an `icu_provider` feature for enabling
    // a constructor for run-time data loading?

    /// Performs the [ToASCII](https://www.unicode.org/reports/tr46/#ToASCII) operation
    /// from UTS #46 with the options indicated.
    ///
    /// # Arguments
    ///
    /// * `domain_name` - The input domain name as UTF-8 bytes. (The UTF-8ness is checked by
    ///   this method and input that is not well-formed UTF-8 is treated as an error. If you
    ///   already have a `&str`, call `.as_bytes()` on it.)
    /// * `ascii_deny_list` - What ASCII deny list, if any, to apply. The UTS 46
    ///   _UseSTD3ASCIIRules_ flag or the WHATWG URL Standard forbidden domain code point
    ///   processing is handled via this argument. Most callers are probably the best off
    ///   by using [`AsciiDenyList::URL`] here.
    /// * `hyphens` - The UTS 46 _CheckHyphens_ flag. Most callers are probably the best
    ///   off by using [`Hyphens::Allow`] here.
    /// * `dns_length` - The UTS 46 _VerifyDNSLength_ flag.
    pub fn to_ascii<'a>(
        &self,
        domain_name: &'a [u8],
        ascii_deny_list: AsciiDenyList,
        hyphens: Hyphens,
        dns_length: DnsLength,
    ) -> Result<Cow<'a, str>, crate::Errors> {
        let mut s = String::new();
        match self.process(
            domain_name,
            ascii_deny_list,
            hyphens,
            ErrorPolicy::FailFast,
            |_, _, _| false,
            &mut s,
            None,
        ) {
            // SAFETY: `ProcessingSuccess::Passthrough` asserts that `domain_name` is ASCII.
            Ok(ProcessingSuccess::Passthrough) => {
                let cow = Cow::Borrowed(unsafe { core::str::from_utf8_unchecked(domain_name) });
                if dns_length != DnsLength::Ignore
                    && !verify_dns_length(&cow, dns_length == DnsLength::VerifyAllowRootDot)
                {
                    Err(crate::Errors::default())
                } else {
                    Ok(cow)
                }
            }
            Ok(ProcessingSuccess::WroteToSink) => {
                let cow: Cow<'_, str> = Cow::Owned(s);
                if dns_length != DnsLength::Ignore
                    && !verify_dns_length(&cow, dns_length == DnsLength::VerifyAllowRootDot)
                {
                    Err(crate::Errors::default())
                } else {
                    Ok(cow)
                }
            }
            Err(ProcessingError::ValidityError) => Err(crate::Errors::default()),
            Err(ProcessingError::SinkError) => unreachable!(),
        }
    }

    /// Performs the [ToUnicode](https://www.unicode.org/reports/tr46/#ToUnicode) operation
    /// from UTS #46 according to the options given. When there
    /// are errors, there is still output, which may be rendered user, even through
    /// the output must not be used in networking protocols. Errors are denoted
    /// by U+FFFD REPLACEMENT CHARACTERs in the output. (That is, if the second item of the
    /// return tuple is `Err`, the first item of the return tuple is guaranteed to contain
    /// at least one U+FFFD.)
    ///
    /// Most applications probably shouldn't use this method and should be using
    /// [`Uts46::to_user_interface`] instead.
    ///
    /// # Arguments
    ///
    /// * `domain_name` - The input domain name as UTF-8 bytes. (The UTF-8ness is checked by
    ///   this method and input that is not well-formed UTF-8 is treated as an error. If you
    ///   already have a `&str`, call `.as_bytes()` on it.)
    /// * `ascii_deny_list` - What ASCII deny list, if any, to apply. The UTS 46
    ///   _UseSTD3ASCIIRules_ flag or the WHATWG URL Standard forbidden domain code point
    ///   processing is handled via this argument. Most callers are probably the best off
    ///   by using [`AsciiDenyList::URL`] here.
    /// * `hyphens` - The UTS 46 _CheckHyphens_ flag. Most callers are probably the best
    ///   off by using [`Hyphens::Allow`] here.
    pub fn to_unicode<'a>(
        &self,
        domain_name: &'a [u8],
        ascii_deny_list: AsciiDenyList,
        hyphens: Hyphens,
    ) -> (Cow<'a, str>, Result<(), crate::Errors>) {
        self.to_user_interface(domain_name, ascii_deny_list, hyphens, |_, _, _| true)
    }

    /// Performs the [ToUnicode](https://www.unicode.org/reports/tr46/#ToUnicode) operation
    /// from UTS #46 according to options given with some
    /// error-free Unicode labels output according to
    /// [ToASCII](https://www.unicode.org/reports/tr46/#ToASCII) instead as decided by
    /// application policy implemented via the `output_as_unicode` closure. The purpose
    /// is to convert user-visible domains to the Unicode form in general but to render
    /// potentially misleading labels as Punycode.
    ///
    /// This is an imperfect security mechanism, because [the Punycode form itself may be
    /// resemble a user-recognizable name](https://www.unicode.org/reports/tr36/#TablePunycodeSpoofing).
    /// However, since this mechanism is common practice, this API provides support for The
    /// the mechanism.
    ///
    /// ASCII labels always pass through as ASCII and labels with errors always pass through
    /// as Unicode. For non-erroneous labels that contain at least one non-ASCII character
    /// (implies non-empty), `output_as_unicode` is called with the Unicode form of the label,
    /// the TLD (potentially empty), and a flag indicating whether the domain name as a whole
    /// is a bidi domain name. If the return value is `true`, the label passes through as
    /// Unicode. If the return value is `false`, the label is converted to Punycode.
    ///
    /// When there are errors, there is still output, which may be rendered user, even through
    /// the output must not be used in networking protocols. Errors are denoted by
    /// U+FFFD REPLACEMENT CHARACTERs in the output. (That is, if the second item
    /// of the return tuple is `Err`, the first item of the return tuple is guaranteed to contain
    /// at least one U+FFFD.) Labels that contain errors are not converted to Punycode.
    ///
    /// # Arguments
    ///
    /// * `domain_name` - The input domain name as UTF-8 bytes. (The UTF-8ness is checked by
    ///   this method and input that is not well-formed UTF-8 is treated as an error. If you
    ///   already have a `&str`, call `.as_bytes()` on it.)
    /// * `ascii_deny_list` - What ASCII deny list, if any, to apply. The UTS 46
    ///   _UseSTD3ASCIIRules_ flag or the WHATWG URL Standard forbidden domain code point
    ///   processing is handled via this argument. Most callers are probably the best off
    ///   by using [`AsciiDenyList::URL`] here.
    /// * `hyphens` - The UTS 46 _CheckHyphens_ flag. Most callers are probably the best
    ///   off by using [`Hyphens::Allow`] here.
    /// * `output_as_unicode` - A closure for deciding if a label should be output as Unicode
    ///   (as opposed to Punycode). The first argument is the label for which a decision is
    ///   needed (always non-empty slice). The second argument is the TLD (potentially empty).
    ///   The third argument is `true` iff the domain name as a whole is a bidi domain name.
    ///   Only non-erroneous labels that contain at least one non-ASCII character are passed
    ///   to the closure as the first argument. The second and third argument values are
    ///   guaranteed to remain the same during a single call to `process`, and the closure
    ///   may cache computations derived from the second and third argument (hence the
    ///   `FnMut` type).
    pub fn to_user_interface<'a, OutputUnicode: FnMut(&[char], &[char], bool) -> bool>(
        &self,
        domain_name: &'a [u8],
        ascii_deny_list: AsciiDenyList,
        hyphens: Hyphens,
        output_as_unicode: OutputUnicode,
    ) -> (Cow<'a, str>, Result<(), crate::Errors>) {
        let mut s = String::new();
        match self.process(
            domain_name,
            ascii_deny_list,
            hyphens,
            ErrorPolicy::MarkErrors,
            output_as_unicode,
            &mut s,
            None,
        ) {
            // SAFETY: `ProcessingSuccess::Passthrough` asserts that `domain_name` is ASCII.
            Ok(ProcessingSuccess::Passthrough) => (
                Cow::Borrowed(unsafe { core::str::from_utf8_unchecked(domain_name) }),
                Ok(()),
            ),
            Ok(ProcessingSuccess::WroteToSink) => (Cow::Owned(s), Ok(())),
            Err(ProcessingError::ValidityError) => (Cow::Owned(s), Err(crate::Errors::default())),
            Err(ProcessingError::SinkError) => unreachable!(),
        }
    }

    /// The lower-level function that [`Uts46::to_ascii`], [`Uts46::to_unicode`], and
    /// [`Uts46::to_user_interface`] are built on to allow support for output types other
    /// than `Cow<'a, str>` (e.g. string types in a non-Rust programming language).
    ///
    /// # Arguments
    ///
    /// * `domain_name` - The input domain name as UTF-8 bytes. (The UTF-8ness is checked by
    ///   this method and input that is not well-formed UTF-8 is treated as an error. If you
    ///   already have a `&str`, call `.as_bytes()` on it.)
    /// * `ascii_deny_list` - What ASCII deny list, if any, to apply. The UTS 46
    ///   _UseSTD3ASCIIRules_ flag or the WHATWG URL Standard forbidden domain code point
    ///   processing is handled via this argument. Most callers are probably the best off
    ///   by using [`AsciiDenyList::URL`] here.
    /// * `hyphens` - The UTS 46 _CheckHyphens_ flag. Most callers are probably the best
    ///   off by using [`Hyphens::Allow`] here.
    /// * `error_policy` - Whether to fail fast or to produce output that may be rendered
    ///   for the user to examine in case of errors.
    /// * `output_as_unicode` - A closure for deciding if a label should be output as Unicode
    ///   (as opposed to Punycode). The first argument is the label for which a decision is
    ///   needed (always non-empty slice). The second argument is the TLD (potentially empty).
    ///   The third argument is `true` iff the domain name as a whole is a bidi domain name.
    ///   Only non-erroneous labels that contain at least one non-ASCII character are passed
    ///   to the closure as the first argument. The second and third argument values are
    ///   guaranteed to remain the same during a single call to `process`, and the closure
    ///   may cache computations derived from the second and third argument (hence the
    ///   `FnMut` type). To perform the _ToASCII_ operation, `|_, _, _| false` must be
    ///   passed as the closure. To perform the _ToUnicode_ operation, `|_, _, _| true` must
    ///   be passed as the closure. A more complex closure may be used to prepare a domain
    ///   name for display in a user interface so that labels are converted to the Unicode
    ///   form in general but potentially misleading labels are converted to the Punycode
    ///   form.
    /// * `sink` - The object that receives the output (in the non-passthrough case).
    /// * `ascii_sink` - A second sink that receives the _ToASCII_ form only if there
    ///   were no errors and `sink` received at least one character of non-ASCII output.
    ///   The purpose of this argument is to enable a user interface display form of the
    ///   domain and the _ToASCII_ form of the domain to be computed efficiently together.
    ///   This argument is useless when `output_as_unicode` always returns `false`, in
    ///   which case the _ToASCII_ form ends up in `sink` already. If `ascii_sink` receives
    ///   no output and the return value is `Ok(ProcessingSuccess::WroteToSink)`, use the
    ///   output received by `sink` also as the _ToASCII_ result.
    ///
    /// # Return value
    ///
    /// * `Ok(ProcessingSuccess::Passthrough)` - The caller must treat
    ///   `unsafe { core::str::from_utf8_unchecked(domain_name) }` as the output. (This
    ///   return value asserts that calling `core::str::from_utf8_unchecked(domain_name)`
    ///   is safe.)
    /// * `Ok(ProcessingSuccess::WroteToSink)` - The caller must treat was was written
    ///   to `sink` as the output. If another sink was passed as `ascii_sink` but it did
    ///   not receive output, the caller must treat what was written to `sink` also as
    ///   the _ToASCII_ output. Otherwise, if `ascii_sink` received output, the caller
    ///   must treat what was written to `ascii_sink` as the _ToASCII_ output.
    /// * `Err(ProcessingError::ValidityError)` - The input was in error and must
    ///   not be used for DNS lookup or otherwise in a network protocol. If `error_policy`
    ///   was `ErrorPolicy::MarkErrors`, the output written to `sink` may be displayed
    ///   to the user as an illustration of where the error was or the errors were.
    /// * `Err(ProcessingError::SinkError)` - Either `sink` or `ascii_sink` returned
    ///   [`core::fmt::Error`]. The partial output written to `sink` `ascii_sink` must not
    ///   be used. If `W` never returns [`core::fmt::Error`], this method never returns
    ///   `Err(ProcessingError::SinkError)`.
    ///
    /// # Safety-usable invariant
    ///
    /// If the return value is `Ok(ProcessingSuccess::Passthrough)`, `domain_name` is
    /// ASCII and `core::str::from_utf8_unchecked(domain_name)` is safe. (Note:
    /// Other return values do _not_ imply that `domain_name` wasn't ASCII!)
    ///
    /// # Security considerations
    ///
    /// Showing labels whose Unicode form might mislead the user as Punycode instead is
    /// an imperfect security mechanism, because [the Punycode form itself may be resemble
    /// a user-recognizable name](https://www.unicode.org/reports/tr36/#TablePunycodeSpoofing).
    /// However, since this mechanism is common practice, this API provides support for the
    /// the mechanism.
    ///
    /// Punycode processing is quadratic, so to avoid denial of service, this method imposes
    /// length limits on Punycode treating especially long inputs as being in error. These
    /// limits are well higher than the DNS length limits and are not more restrictive than
    /// the limits imposed by ICU4C.
    #[allow(clippy::too_many_arguments)]
    pub fn process<W: Write + ?Sized, OutputUnicode: FnMut(&[char], &[char], bool) -> bool>(
        &self,
        domain_name: &[u8],
        ascii_deny_list: AsciiDenyList,
        hyphens: Hyphens,
        error_policy: ErrorPolicy,
        mut output_as_unicode: OutputUnicode,
        sink: &mut W,
        ascii_sink: Option<&mut W>,
    ) -> Result<ProcessingSuccess, ProcessingError> {
        let fail_fast = error_policy == ErrorPolicy::FailFast;
        let mut domain_buffer = SmallVec::<[char; 253]>::new();
        let mut already_punycode = SmallVec::<[AlreadyAsciiLabel; 8]>::new();
        // `process_inner` could be pasted inline here, but it's out of line in order
        // to avoid duplicating that code when monomorphizing over `W` and `OutputUnicode`.
        let (passthrough_up_to, is_bidi, had_errors) = self.process_inner(
            domain_name,
            ascii_deny_list,
            hyphens,
            fail_fast,
            &mut domain_buffer,
            &mut already_punycode,
        );
        if passthrough_up_to == domain_name.len() {
            debug_assert!(!had_errors);
            return Ok(ProcessingSuccess::Passthrough);
        }
        // Checked only after passthrough as a micro optimization.
        if fail_fast && had_errors {
            return Err(ProcessingError::ValidityError);
        }
        debug_assert_eq!(had_errors, domain_buffer.contains(&'\u{FFFD}'));
        let without_dot = if let Some(without_dot) = domain_buffer.strip_suffix(&['.']) {
            without_dot
        } else {
            &domain_buffer[..]
        };
        // unwrap is OK, because we always have at least one label
        let tld = without_dot.rsplit(|c| *c == '.').next().unwrap();
        let mut had_unicode_output = false;
        let mut seen_label = false;
        let mut already_punycode_iter = already_punycode.iter();
        let mut passthrough_up_to_extended = passthrough_up_to;
        let mut flushed_prefix = false;
        for label in domain_buffer.split(|c| *c == '.') {
            // Unwrap is OK, because there are supposed to be as many items in
            // `already_punycode` as there are labels.
            let input_punycode = *already_punycode_iter.next().unwrap();
            if seen_label {
                if flushed_prefix {
                    sink.write_char('.')?;
                } else {
                    debug_assert_eq!(domain_name[passthrough_up_to_extended], b'.');
                    passthrough_up_to_extended += 1;
                    if passthrough_up_to_extended == domain_name.len() {
                        debug_assert!(!had_errors);
                        return Ok(ProcessingSuccess::Passthrough);
                    }
                }
            }
            seen_label = true;

            if let AlreadyAsciiLabel::MixedCaseAscii(mixed_case) = input_punycode {
                if let Some(first_upper_case) =
                    mixed_case.iter().position(|c| c.is_ascii_uppercase())
                {
                    let (head, tail) = mixed_case.split_at(first_upper_case);
                    let slice_to_write = if flushed_prefix {
                        head
                    } else {
                        flushed_prefix = true;
                        passthrough_up_to_extended += head.len();
                        debug_assert_ne!(passthrough_up_to_extended, domain_name.len());
                        &domain_name[..passthrough_up_to_extended]
                    };
                    // SAFETY: `mixed_case` and `domain_name` up to `passthrough_up_to_extended` are known to be ASCII.
                    sink.write_str(unsafe { core::str::from_utf8_unchecked(slice_to_write) })?;
                    for c in tail.iter() {
                        sink.write_char(char::from(c.to_ascii_lowercase()))?;
                    }
                } else if flushed_prefix {
                    // SAFETY: `mixed_case` is known to be ASCII.
                    sink.write_str(unsafe { core::str::from_utf8_unchecked(mixed_case) })?;
                } else {
                    passthrough_up_to_extended += mixed_case.len();
                    if passthrough_up_to_extended == domain_name.len() {
                        debug_assert!(!had_errors);
                        return Ok(ProcessingSuccess::Passthrough);
                    }
                }
                continue;
            }

            let potentially_punycode = if fail_fast {
                debug_assert!(classify_for_punycode(label) != PunycodeClassification::Error);
                !is_ascii(label)
            } else {
                classify_for_punycode(label) == PunycodeClassification::Unicode
            };
            let passthrough = if potentially_punycode {
                let unicode = output_as_unicode(label, tld, is_bidi);
                had_unicode_output |= unicode;
                unicode
            } else {
                true
            };
            if passthrough {
                if !flushed_prefix {
                    flushed_prefix = true;
                    // SAFETY: `domain_name` up to `passthrough_up_to_extended` is known to be ASCII.
                    sink.write_str(unsafe {
                        core::str::from_utf8_unchecked(&domain_name[..passthrough_up_to_extended])
                    })?;
                }
                for c in label.iter().copied() {
                    sink.write_char(c)?;
                }
            } else if let AlreadyAsciiLabel::MixedCasePunycode(mixed_case) = input_punycode {
                if let Some(first_upper_case) =
                    mixed_case.iter().position(|c| c.is_ascii_uppercase())
                {
                    let (head, tail) = mixed_case.split_at(first_upper_case);
                    let slice_to_write = if flushed_prefix {
                        head
                    } else {
                        flushed_prefix = true;
                        passthrough_up_to_extended += head.len();
                        debug_assert_ne!(passthrough_up_to_extended, domain_name.len());
                        &domain_name[..passthrough_up_to_extended]
                    };
                    // SAFETY: `mixed_case` and `domain_name` up to `passthrough_up_to_extended` are known to be ASCII.
                    sink.write_str(unsafe { core::str::from_utf8_unchecked(slice_to_write) })?;
                    for c in tail.iter() {
                        sink.write_char(char::from(c.to_ascii_lowercase()))?;
                    }
                } else if flushed_prefix {
                    // SAFETY: `mixed_case` is known to be ASCII.
                    sink.write_str(unsafe { core::str::from_utf8_unchecked(mixed_case) })?;
                } else {
                    passthrough_up_to_extended += mixed_case.len();
                    if passthrough_up_to_extended == domain_name.len() {
                        debug_assert!(!had_errors);
                        return Ok(ProcessingSuccess::Passthrough);
                    }
                }
            } else {
                if !flushed_prefix {
                    flushed_prefix = true;
                    // SAFETY: `domain_name` up to `passthrough_up_to_extended` is known to be ASCII.
                    sink.write_str(unsafe {
                        core::str::from_utf8_unchecked(&domain_name[..passthrough_up_to_extended])
                    })?;
                }
                write_punycode_label(label, sink)?;
            }
        }

        if had_errors {
            return Err(ProcessingError::ValidityError);
        }

        if had_unicode_output {
            if let Some(sink) = ascii_sink {
                let mut seen_label = false;
                let mut already_punycode_iter = already_punycode.iter();
                let mut passthrough_up_to_extended = passthrough_up_to;
                let mut flushed_prefix = false;
                for label in domain_buffer.split(|c| *c == '.') {
                    // Unwrap is OK, because there are supposed to be as many items in
                    // `already_punycode` as there are labels.
                    let input_punycode = *already_punycode_iter.next().unwrap();
                    if seen_label {
                        if flushed_prefix {
                            sink.write_char('.')?;
                        } else {
                            debug_assert_eq!(domain_name[passthrough_up_to_extended], b'.');
                            passthrough_up_to_extended += 1;
                        }
                    }
                    seen_label = true;

                    if let AlreadyAsciiLabel::MixedCaseAscii(mixed_case) = input_punycode {
                        if let Some(first_upper_case) =
                            mixed_case.iter().position(|c| c.is_ascii_uppercase())
                        {
                            let (head, tail) = mixed_case.split_at(first_upper_case);
                            let slice_to_write = if flushed_prefix {
                                head
                            } else {
                                flushed_prefix = true;
                                passthrough_up_to_extended += head.len();
                                debug_assert_ne!(passthrough_up_to_extended, domain_name.len());
                                &domain_name[..passthrough_up_to_extended]
                            };
                            // SAFETY: `mixed_case` and `domain_name` up to `passthrough_up_to_extended` are known to be ASCII.
                            sink.write_str(unsafe {
                                core::str::from_utf8_unchecked(slice_to_write)
                            })?;
                            for c in tail.iter() {
                                sink.write_char(char::from(c.to_ascii_lowercase()))?;
                            }
                        } else if flushed_prefix {
                            // SAFETY: `mixed_case` is known to be ASCII.
                            sink.write_str(unsafe { core::str::from_utf8_unchecked(mixed_case) })?;
                        } else {
                            passthrough_up_to_extended += mixed_case.len();
                        }
                        continue;
                    }

                    if is_ascii(label) {
                        if !flushed_prefix {
                            flushed_prefix = true;
                            // SAFETY: `domain_name` up to `passthrough_up_to_extended` is known to be ASCII.
                            sink.write_str(unsafe {
                                core::str::from_utf8_unchecked(
                                    &domain_name[..passthrough_up_to_extended],
                                )
                            })?;
                        }
                        for c in label.iter().copied() {
                            sink.write_char(c)?;
                        }
                    } else if let AlreadyAsciiLabel::MixedCasePunycode(mixed_case) = input_punycode
                    {
                        if let Some(first_upper_case) =
                            mixed_case.iter().position(|c| c.is_ascii_uppercase())
                        {
                            let (head, tail) = mixed_case.split_at(first_upper_case);
                            let slice_to_write = if flushed_prefix {
                                head
                            } else {
                                flushed_prefix = true;
                                passthrough_up_to_extended += head.len();
                                debug_assert_ne!(passthrough_up_to_extended, domain_name.len());
                                &domain_name[..passthrough_up_to_extended]
                            };
                            // SAFETY: `mixed_case` and `domain_name` up to `passthrough_up_to_extended` are known to be ASCII.
                            sink.write_str(unsafe {
                                core::str::from_utf8_unchecked(slice_to_write)
                            })?;
                            for c in tail.iter() {
                                sink.write_char(char::from(c.to_ascii_lowercase()))?;
                            }
                        } else if flushed_prefix {
                            // SAFETY: `mixed_case` is known to be ASCII.
                            sink.write_str(unsafe { core::str::from_utf8_unchecked(mixed_case) })?;
                        } else {
                            passthrough_up_to_extended += mixed_case.len();
                        }
                    } else {
                        if !flushed_prefix {
                            flushed_prefix = true;
                            // SAFETY: `domain_name` up to `passthrough_up_to_extended` is known to be ASCII.
                            sink.write_str(unsafe {
                                core::str::from_utf8_unchecked(
                                    &domain_name[..passthrough_up_to_extended],
                                )
                            })?;
                        }
                        write_punycode_label(label, sink)?;
                    }
                }
                if !flushed_prefix {
                    // SAFETY: `domain_name` up to `passthrough_up_to_extended` is known to be ASCII.
                    sink.write_str(unsafe {
                        core::str::from_utf8_unchecked(&domain_name[..passthrough_up_to_extended])
                    })?;
                }
            }
        }
        Ok(ProcessingSuccess::WroteToSink)
    }

    /// The part of `process` that doesn't need to be generic over the sink.
    #[inline(always)]
    fn process_inner<'a>(
        &self,
        domain_name: &'a [u8],
        ascii_deny_list: AsciiDenyList,
        hyphens: Hyphens,
        fail_fast: bool,
        domain_buffer: &mut SmallVec<[char; 253]>,
        already_punycode: &mut SmallVec<[AlreadyAsciiLabel<'a>; 8]>,
    ) -> (usize, bool, bool) {
        // Sadly, this even faster-path ASCII tier is needed to avoid regressing
        // performance.
        let mut iter = domain_name.iter();
        let mut most_recent_label_start = iter.clone();
        loop {
            if let Some(&b) = iter.next() {
                if in_inclusive_range8(b, b'a', b'z') {
                    continue;
                }
                if b == b'.' {
                    most_recent_label_start = iter.clone();
                    continue;
                }
                return self.process_innermost(
                    domain_name,
                    ascii_deny_list,
                    hyphens,
                    fail_fast,
                    domain_buffer,
                    already_punycode,
                    most_recent_label_start.as_slice(),
                );
            } else {
                // Success! The whole input passes through on the fastest path!
                return (domain_name.len(), false, false);
            }
        }
    }

    /// The part of `process` that doesn't need to be generic over the sink and
    /// can avoid monomorphizing in the interest of code size.
    /// Separating this into a different stack frame compared to `process_inner`
    /// improves performance in the ICU4X case.
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    fn process_innermost<'a>(
        &self,
        domain_name: &'a [u8],
        ascii_deny_list: AsciiDenyList,
        hyphens: Hyphens,
        fail_fast: bool,
        domain_buffer: &mut SmallVec<[char; 253]>,
        already_punycode: &mut SmallVec<[AlreadyAsciiLabel<'a>; 8]>,
        tail: &'a [u8],
    ) -> (usize, bool, bool) {
        let deny_list = ascii_deny_list.bits;
        let deny_list_deny_dot = deny_list | DOT_MASK;

        let mut had_errors = false;

        let mut passthrough_up_to = domain_name.len() - tail.len(); // Index into `domain_name`
                                                                    // 253 ASCII characters is the max length for a valid domain name
                                                                    // (excluding the root dot).
        let mut current_label_start; // Index into `domain_buffer`
        let mut seen_label = false;
        let mut in_prefix = true;
        for label in tail.split(|b| *b == b'.') {
            // We check for passthrough only for the prefix. That is, if we
            // haven't moved on and started filling `domain_buffer`. Keeping
            // this stuff in one loop where the first items keep being skipped
            // once they have been skipped at least once instead of working
            // this into a fancier loop structure in order to make sure that
            // no item from the iterator is lost or processed twice.
            // Furthermore, after the passthrough fails, restarting the
            // normalization process after each pre-existing ASCII dot also
            // provides an opportunity for the processing to get back onto
            // an ASCII fast path that bypasses the normalizer for ASCII
            // after a pre-existing ASCII dot (pre-existing in the sense
            // of not coming from e.g. normalizing an ideographic dot).
            if in_prefix && is_passthrough_ascii_label(label) {
                if seen_label {
                    debug_assert_eq!(domain_name[passthrough_up_to], b'.');
                    passthrough_up_to += 1;
                }
                seen_label = true;

                passthrough_up_to += label.len();
                continue;
            }
            if seen_label {
                if in_prefix {
                    debug_assert_eq!(domain_name[passthrough_up_to], b'.');
                    passthrough_up_to += 1;
                } else {
                    domain_buffer.push('.');
                }
            }
            seen_label = true;
            in_prefix = false;
            current_label_start = domain_buffer.len();
            if !label.is_empty() {
                let (ascii, non_ascii) = split_ascii_fast_path_prefix(label);
                let non_punycode_ascii_label = if non_ascii.is_empty() {
                    if has_punycode_prefix(ascii) {
                        if (ascii.last() != Some(&b'-'))
                            && (ascii.len() - 4 <= PUNYCODE_DECODE_MAX_INPUT_LENGTH)
                        {
                            if let Ok(decode) =
                                Decoder::default().decode::<u8, InternalCaller>(&ascii[4..])
                            {
                                // 63 ASCII characters is the max length for a valid DNS label and xn-- takes 4
                                // characters.
                                let mut label_buffer = SmallVec::<[char; 59]>::new();
                                label_buffer.extend(decode);

                                if self.after_punycode_decode(
                                    domain_buffer,
                                    current_label_start,
                                    &label_buffer,
                                    deny_list_deny_dot,
                                    fail_fast,
                                    &mut had_errors,
                                ) {
                                    return (0, false, true);
                                }

                                if self.check_label(
                                    hyphens,
                                    &mut domain_buffer[current_label_start..],
                                    fail_fast,
                                    &mut had_errors,
                                    true,
                                    true,
                                ) {
                                    return (0, false, true);
                                }
                            } else {
                                // Punycode failed
                                if fail_fast {
                                    return (0, false, true);
                                }
                                had_errors = true;
                                domain_buffer.push('\u{FFFD}');
                                let mut iter = ascii.iter();
                                // Discard the first character that we replaced.
                                let _ = iter.next();
                                domain_buffer.extend(iter.map(|c| {
                                    // Can't have dot here, so `deny_list` vs `deny_list_deny_dot` does
                                    // not matter.
                                    apply_ascii_deny_list_to_potentially_upper_case_ascii(
                                        *c, deny_list,
                                    )
                                }));
                            };
                            // If there were errors, we won't be trying to use this
                            // anyway later, so it's fine to put it here unconditionally.
                            already_punycode.push(AlreadyAsciiLabel::MixedCasePunycode(label));
                            continue;
                        } else if fail_fast {
                            return (0, false, true);
                        }
                        // Else fall through to the complex path and rediscover error
                        // there.
                        false
                    } else {
                        true
                    }
                } else {
                    false
                };
                for c in ascii.iter().map(|c| {
                    // Can't have dot here, so `deny_list` vs `deny_list_deny_dot` does
                    // not matter.
                    apply_ascii_deny_list_to_potentially_upper_case_ascii(*c, deny_list)
                }) {
                    if c == '\u{FFFD}' {
                        if fail_fast {
                            return (0, false, true);
                        }
                        had_errors = true;
                    }
                    domain_buffer.push(c);
                }
                if non_punycode_ascii_label {
                    if hyphens != Hyphens::Allow
                        && check_hyphens(
                            &mut domain_buffer[current_label_start..],
                            hyphens == Hyphens::CheckFirstLast,
                            fail_fast,
                            &mut had_errors,
                        )
                    {
                        return (0, false, true);
                    }
                    already_punycode.push(if had_errors {
                        AlreadyAsciiLabel::Other
                    } else {
                        AlreadyAsciiLabel::MixedCaseAscii(label)
                    });
                    continue;
                }
                already_punycode.push(AlreadyAsciiLabel::Other);
                let mut first_needs_combining_mark_check = ascii.is_empty();
                let mut needs_contextj_check = !non_ascii.is_empty();
                let mut mapping = self
                    .data
                    .map_normalize(non_ascii.chars())
                    .map(|c| apply_ascii_deny_list_to_lower_cased_unicode(c, deny_list));
                loop {
                    let n = mapping.next();
                    match n {
                        None | Some('.') => {
                            if domain_buffer[current_label_start..]
                                .starts_with(&['x', 'n', '-', '-'])
                            {
                                let mut punycode_precondition_failed = false;
                                for c in domain_buffer[current_label_start + 4..].iter_mut() {
                                    if !c.is_ascii() {
                                        if fail_fast {
                                            return (0, false, true);
                                        }
                                        had_errors = true;
                                        *c = '\u{FFFD}';
                                        punycode_precondition_failed = true;
                                    }
                                }

                                if let Some(last) = domain_buffer.last_mut() {
                                    if *last == '-' {
                                        // Either there's nothing after the "xn--" prefix
                                        // and we got the last hyphen of "xn--", or there
                                        // are no Punycode digits after the last delimiter
                                        // which would result in Punycode decode outputting
                                        // ASCII only.
                                        if fail_fast {
                                            return (0, false, true);
                                        }
                                        had_errors = true;
                                        *last = '\u{FFFD}';
                                        punycode_precondition_failed = true;
                                    }
                                } else {
                                    unreachable!();
                                }

                                // Reject excessively long input
                                // https://github.com/whatwg/url/issues/824
                                // https://unicode-org.atlassian.net/browse/ICU-13727
                                if domain_buffer.len() - current_label_start - 4
                                    > PUNYCODE_DECODE_MAX_INPUT_LENGTH
                                {
                                    if fail_fast {
                                        return (0, false, true);
                                    }
                                    had_errors = true;
                                    domain_buffer[current_label_start
                                        + 4
                                        + PUNYCODE_DECODE_MAX_INPUT_LENGTH] = '\u{FFFD}';
                                    punycode_precondition_failed = true;
                                }

                                if !punycode_precondition_failed {
                                    if let Ok(decode) = Decoder::default()
                                        .decode::<char, InternalCaller>(
                                            &domain_buffer[current_label_start + 4..],
                                        )
                                    {
                                        first_needs_combining_mark_check = true;
                                        needs_contextj_check = true;
                                        // 63 ASCII characters is the max length for a valid DNS label and xn-- takes 4
                                        // characters.
                                        let mut label_buffer = SmallVec::<[char; 59]>::new();
                                        label_buffer.extend(decode);

                                        domain_buffer.truncate(current_label_start);
                                        if self.after_punycode_decode(
                                            domain_buffer,
                                            current_label_start,
                                            &label_buffer,
                                            deny_list_deny_dot,
                                            fail_fast,
                                            &mut had_errors,
                                        ) {
                                            return (0, false, true);
                                        }
                                    } else {
                                        // Punycode failed
                                        if fail_fast {
                                            return (0, false, true);
                                        }
                                        had_errors = true;
                                        domain_buffer[current_label_start] = '\u{FFFD}';
                                        needs_contextj_check = false; // ASCII label
                                        first_needs_combining_mark_check = false;
                                    };
                                } else {
                                    first_needs_combining_mark_check = false;
                                    needs_contextj_check = false; // Non-ASCII already turned to U+FFFD.
                                }
                            }
                            if self.check_label(
                                hyphens,
                                &mut domain_buffer[current_label_start..],
                                fail_fast,
                                &mut had_errors,
                                first_needs_combining_mark_check,
                                needs_contextj_check,
                            ) {
                                return (0, false, true);
                            }

                            if n.is_none() {
                                break;
                            }
                            domain_buffer.push('.');
                            current_label_start = domain_buffer.len();
                            first_needs_combining_mark_check = true;
                            needs_contextj_check = true;
                            already_punycode.push(AlreadyAsciiLabel::Other);
                        }
                        Some(c) => {
                            if c == '\u{FFFD}' {
                                if fail_fast {
                                    return (0, false, true);
                                }
                                had_errors = true;
                            }
                            domain_buffer.push(c);
                        }
                    }
                }
            } else {
                // Empty label
                already_punycode.push(AlreadyAsciiLabel::MixedCaseAscii(label));
            }
        }

        let is_bidi = self.is_bidi(domain_buffer);
        if is_bidi {
            for label in domain_buffer.split_mut(|c| *c == '.') {
                if let Some((first, tail)) = label.split_first_mut() {
                    let first_bc = self.data.bidi_class(*first);
                    if !FIRST_BC_MASK.intersects(first_bc.to_mask()) {
                        // Neither RTL label nor LTR label
                        if fail_fast {
                            return (0, false, true);
                        }
                        had_errors = true;
                        *first = '\u{FFFD}';
                        continue;
                    }
                    let is_ltr = first_bc.is_ltr();
                    // Trim NSM
                    let mut middle = tail;
                    #[allow(clippy::while_let_loop)]
                    loop {
                        if let Some((last, prior)) = middle.split_last_mut() {
                            let last_bc = self.data.bidi_class(*last);
                            if last_bc.is_nonspacing_mark() {
                                middle = prior;
                                continue;
                            }
                            let last_mask = if is_ltr { LAST_LTR_MASK } else { LAST_RTL_MASK };
                            if !last_mask.intersects(last_bc.to_mask()) {
                                if fail_fast {
                                    return (0, false, true);
                                }
                                had_errors = true;
                                *last = '\u{FFFD}';
                            }
                            if is_ltr {
                                for c in prior.iter_mut() {
                                    let bc = self.data.bidi_class(*c);
                                    if !MIDDLE_LTR_MASK.intersects(bc.to_mask()) {
                                        if fail_fast {
                                            return (0, false, true);
                                        }
                                        had_errors = true;
                                        *c = '\u{FFFD}';
                                    }
                                }
                            } else {
                                let mut numeral_state = RtlNumeralState::Undecided;
                                for c in prior.iter_mut() {
                                    let bc = self.data.bidi_class(*c);
                                    if !MIDDLE_RTL_MASK.intersects(bc.to_mask()) {
                                        if fail_fast {
                                            return (0, false, true);
                                        }
                                        had_errors = true;
                                        *c = '\u{FFFD}';
                                    } else {
                                        match numeral_state {
                                            RtlNumeralState::Undecided => {
                                                if bc.is_european_number() {
                                                    numeral_state = RtlNumeralState::European;
                                                } else if bc.is_arabic_number() {
                                                    numeral_state = RtlNumeralState::Arabic;
                                                }
                                            }
                                            RtlNumeralState::European => {
                                                if bc.is_arabic_number() {
                                                    if fail_fast {
                                                        return (0, false, true);
                                                    }
                                                    had_errors = true;
                                                    *c = '\u{FFFD}';
                                                }
                                            }
                                            RtlNumeralState::Arabic => {
                                                if bc.is_european_number() {
                                                    if fail_fast {
                                                        return (0, false, true);
                                                    }
                                                    had_errors = true;
                                                    *c = '\u{FFFD}';
                                                }
                                            }
                                        }
                                    }
                                }
                                if (numeral_state == RtlNumeralState::European
                                    && last_bc.is_arabic_number())
                                    || (numeral_state == RtlNumeralState::Arabic
                                        && last_bc.is_european_number())
                                {
                                    if fail_fast {
                                        return (0, false, true);
                                    }
                                    had_errors = true;
                                    *last = '\u{FFFD}';
                                }
                            }
                            break;
                        } else {
                            // One-character label or label where
                            // everything after the first character
                            // is just non-spacing marks.
                            break;
                        }
                    }
                }
            }
        }

        (passthrough_up_to, is_bidi, had_errors)
    }

    #[inline(never)]
    fn after_punycode_decode(
        &self,
        domain_buffer: &mut SmallVec<[char; 253]>,
        current_label_start: usize,
        label_buffer: &[char],
        deny_list_deny_dot: u128,
        fail_fast: bool,
        had_errors: &mut bool,
    ) -> bool {
        for c in self
            .data
            .normalize_validate(label_buffer.iter().copied())
            .map(|c| apply_ascii_deny_list_to_lower_cased_unicode(c, deny_list_deny_dot))
        {
            if c == '\u{FFFD}' {
                if fail_fast {
                    return true;
                }
                *had_errors = true;
            }
            domain_buffer.push(c);
        }
        let normalized = &mut domain_buffer[current_label_start..];
        if let Err(()) =
            normalized
                .iter_mut()
                .zip(label_buffer.iter())
                .try_for_each(|(norm_c, decoded_c)| {
                    if *norm_c == *decoded_c {
                        Ok(())
                    } else {
                        // Mark the first difference
                        *norm_c = '\u{FFFD}';
                        Err(())
                    }
                })
        {
            if fail_fast {
                return true;
            }
            *had_errors = true;
        }
        false
    }

    #[inline(never)]
    fn check_label(
        &self,
        hyphens: Hyphens,
        mut_label: &mut [char],
        fail_fast: bool,
        had_errors: &mut bool,
        first_needs_combining_mark_check: bool,
        needs_contextj_check: bool,
    ) -> bool {
        if hyphens != Hyphens::Allow
            && check_hyphens(
                mut_label,
                hyphens == Hyphens::CheckFirstLast,
                fail_fast,
                had_errors,
            )
        {
            return true;
        }
        if first_needs_combining_mark_check {
            if let Some(first) = mut_label.first_mut() {
                if self.data.is_mark(*first) {
                    if fail_fast {
                        return true;
                    }
                    *had_errors = true;
                    *first = '\u{FFFD}';
                }
            }
        }
        if needs_contextj_check {
            // ContextJ
            for i in 0..mut_label.len() {
                let c = mut_label[i];
                if !in_inclusive_range_char(c, '\u{200C}', '\u{200D}') {
                    continue;
                }
                let (head, joiner_and_tail) = mut_label.split_at_mut(i);

                if let Some((joiner, tail)) = joiner_and_tail.split_first_mut() {
                    if let Some(previous) = head.last() {
                        if self.data.is_virama(*previous) {
                            continue;
                        }
                    } else {
                        // No preceding character
                        if fail_fast {
                            return true;
                        }
                        *had_errors = true;
                        *joiner = '\u{FFFD}';
                        continue;
                    }
                    if c == '\u{200D}' {
                        // ZWJ only has the virama rule
                        if fail_fast {
                            return true;
                        }
                        *had_errors = true;
                        *joiner = '\u{FFFD}';
                        continue;
                    }
                    debug_assert_eq!(c, '\u{200C}');
                    if !self.has_appropriately_joining_char(
                        head.iter().rev().copied(),
                        LEFT_OR_DUAL_JOINING_MASK,
                    ) || !self.has_appropriately_joining_char(
                        tail.iter().copied(),
                        RIGHT_OR_DUAL_JOINING_MASK,
                    ) {
                        if fail_fast {
                            return true;
                        }
                        *had_errors = true;
                        *joiner = '\u{FFFD}';
                    }
                } else {
                    debug_assert!(false);
                }
            }
        }

        if !is_ascii(mut_label) && mut_label.len() > PUNYCODE_ENCODE_MAX_INPUT_LENGTH {
            // Limit quadratic behavior
            // https://github.com/whatwg/url/issues/824
            // https://unicode-org.atlassian.net/browse/ICU-13727
            if fail_fast {
                return true;
            }
            *had_errors = true;
            mut_label[PUNYCODE_ENCODE_MAX_INPUT_LENGTH] = '\u{FFFD}';
        }
        false
    }

    #[inline(always)]
    fn has_appropriately_joining_char<I: Iterator<Item = char>>(
        &self,
        iter: I,
        required_mask: JoiningTypeMask,
    ) -> bool {
        for c in iter {
            let jt = self.data.joining_type(c);
            if jt.to_mask().intersects(required_mask) {
                return true;
            }
            if jt.is_transparent() {
                continue;
            }
            return false;
        }
        false
    }

    #[inline(always)]
    fn is_bidi(&self, buffer: &[char]) -> bool {
        for &c in buffer {
            if c < '\u{0590}' {
                // Below Hebrew
                continue;
            }
            if in_inclusive_range_char(c, '\u{0900}', '\u{FB1C}') {
                debug_assert_ne!(c, '\u{200F}'); // disallowed
                continue;
            }
            if in_inclusive_range_char(c, '\u{1F000}', '\u{3FFFF}') {
                continue;
            }
            if in_inclusive_range_char(c, '\u{FF00}', '\u{107FF}') {
                continue;
            }
            if in_inclusive_range_char(c, '\u{11000}', '\u{1E7FF}') {
                continue;
            }
            if RTL_MASK.intersects(self.data.bidi_class(c).to_mask()) {
                return true;
            }
        }
        false
    }
}

fn check_hyphens(
    mut_label: &mut [char],
    allow_third_fourth: bool,
    fail_fast: bool,
    had_errors: &mut bool,
) -> bool {
    if let Some(first) = mut_label.first_mut() {
        if *first == '-' {
            if fail_fast {
                return true;
            }
            *had_errors = true;
            *first = '\u{FFFD}';
        }
    }
    if let Some(last) = mut_label.last_mut() {
        if *last == '-' {
            if fail_fast {
                return true;
            }
            *had_errors = true;
            *last = '\u{FFFD}';
        }
    }
    if allow_third_fourth {
        return false;
    }
    if mut_label.len() >= 4 && mut_label[2] == '-' && mut_label[3] == '-' {
        if fail_fast {
            return true;
        }
        *had_errors = true;
        mut_label[2] = '\u{FFFD}';
        mut_label[3] = '\u{FFFD}';
    }
    false
}
