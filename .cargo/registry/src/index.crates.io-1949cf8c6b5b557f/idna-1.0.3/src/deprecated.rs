// Copyright 2013-2014 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Deprecated API for [*Unicode IDNA Compatibility Processing*
//! (Unicode Technical Standard #46)](http://www.unicode.org/reports/tr46/)

#![allow(deprecated)]

use alloc::borrow::Cow;
use alloc::string::String;

use crate::uts46::*;
use crate::Errors;

/// Performs preprocessing equivalent to UTS 46 transitional processing
/// if `transitional` is `true`. If `transitional` is `false`, merely
/// lets the input pass through as-is (for call site convenience).
///
/// The output of this function is to be passed to [`Uts46::process`].
fn map_transitional(domain: &str, transitional: bool) -> Cow<'_, str> {
    if !transitional {
        return Cow::Borrowed(domain);
    }
    let mut chars = domain.chars();
    loop {
        let prev = chars.clone();
        if let Some(c) = chars.next() {
            match c {
                'ß' | 'ẞ' | 'ς' | '\u{200C}' | '\u{200D}' => {
                    let mut s = String::with_capacity(domain.len());
                    let tail = prev.as_str();
                    let head = &domain[..domain.len() - tail.len()];
                    s.push_str(head);
                    for c in tail.chars() {
                        match c {
                            'ß' | 'ẞ' => {
                                s.push_str("ss");
                            }
                            'ς' => {
                                s.push('σ');
                            }
                            '\u{200C}' | '\u{200D}' => {}
                            _ => {
                                s.push(c);
                            }
                        }
                    }
                    return Cow::Owned(s);
                }
                _ => {}
            }
        } else {
            break;
        }
    }
    Cow::Borrowed(domain)
}

/// Deprecated. Use the crate-top-level functions or [`Uts46`].
#[derive(Default)]
#[deprecated]
pub struct Idna {
    config: Config,
}

impl Idna {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// [UTS 46 ToASCII](http://www.unicode.org/reports/tr46/#ToASCII)
    #[allow(clippy::wrong_self_convention)] // Retain old weirdness in deprecated API
    pub fn to_ascii(&mut self, domain: &str, out: &mut String) -> Result<(), Errors> {
        let mapped = map_transitional(domain, self.config.transitional_processing);
        match Uts46::new().process(
            mapped.as_bytes(),
            self.config.deny_list(),
            self.config.hyphens(),
            ErrorPolicy::FailFast, // Old code did not appear to expect the output to be useful in the error case.
            |_, _, _| false,
            out,
            None,
        ) {
            Ok(ProcessingSuccess::Passthrough) => {
                if self.config.verify_dns_length && !verify_dns_length(&mapped, true) {
                    return Err(crate::Errors::default());
                }
                out.push_str(&mapped);
                Ok(())
            }
            Ok(ProcessingSuccess::WroteToSink) => {
                if self.config.verify_dns_length && !verify_dns_length(out, true) {
                    return Err(crate::Errors::default());
                }
                Ok(())
            }
            Err(ProcessingError::ValidityError) => Err(crate::Errors::default()),
            Err(ProcessingError::SinkError) => unreachable!(),
        }
    }

    /// [UTS 46 ToUnicode](http://www.unicode.org/reports/tr46/#ToUnicode)
    #[allow(clippy::wrong_self_convention)] // Retain old weirdness in deprecated API
    pub fn to_unicode(&mut self, domain: &str, out: &mut String) -> Result<(), Errors> {
        let mapped = map_transitional(domain, self.config.transitional_processing);
        match Uts46::new().process(
            mapped.as_bytes(),
            self.config.deny_list(),
            self.config.hyphens(),
            ErrorPolicy::MarkErrors,
            |_, _, _| true,
            out,
            None,
        ) {
            Ok(ProcessingSuccess::Passthrough) => {
                out.push_str(&mapped);
                Ok(())
            }
            Ok(ProcessingSuccess::WroteToSink) => Ok(()),
            Err(ProcessingError::ValidityError) => Err(crate::Errors::default()),
            Err(ProcessingError::SinkError) => unreachable!(),
        }
    }
}

/// Deprecated configuration API.
#[derive(Clone, Copy)]
#[must_use]
#[deprecated]
pub struct Config {
    use_std3_ascii_rules: bool,
    transitional_processing: bool,
    verify_dns_length: bool,
    check_hyphens: bool,
}

/// The defaults are that of _beStrict=false_ in the [WHATWG URL Standard](https://url.spec.whatwg.org/#idna)
impl Default for Config {
    fn default() -> Self {
        Config {
            use_std3_ascii_rules: false,
            transitional_processing: false,
            check_hyphens: false,
            // Only use for to_ascii, not to_unicode
            verify_dns_length: false,
        }
    }
}

impl Config {
    /// Whether to enforce STD3 or WHATWG URL Standard ASCII deny list.
    ///
    /// `true` for STD3, `false` for no deny list.
    ///
    /// Note that `true` rejects pseudo-hosts used by various TXT record-based protocols.
    #[inline]
    pub fn use_std3_ascii_rules(mut self, value: bool) -> Self {
        self.use_std3_ascii_rules = value;
        self
    }

    /// Whether to enable (deprecated) transitional processing.
    ///
    /// Note that Firefox, Safari, and Chrome do not use transitional
    /// processing.
    #[inline]
    pub fn transitional_processing(mut self, value: bool) -> Self {
        self.transitional_processing = value;
        self
    }

    /// Whether the _VerifyDNSLength_ operation should be performed
    /// by `to_ascii`.
    ///
    /// For compatibility with previous behavior, even when set to `true`,
    /// the trailing root label dot is allowed contrary to the spec.
    #[inline]
    pub fn verify_dns_length(mut self, value: bool) -> Self {
        self.verify_dns_length = value;
        self
    }

    /// Whether to enforce STD3 rules for hyphen placement.
    ///
    /// `true` to deny hyphens in the first and last positions.
    /// `false` to not enforce hyphen placement.
    ///
    /// Note that for backward compatibility this is not the same as
    /// UTS 46 _CheckHyphens_, which also disallows hyphens in the
    /// third and fourth positions.
    ///
    /// Note that `true` rejects real-world names, including some GitHub user pages.
    #[inline]
    pub fn check_hyphens(mut self, value: bool) -> Self {
        self.check_hyphens = value;
        self
    }

    /// Obsolete method retained to ease migration. The argument must be `false`.
    ///
    /// Panics
    ///
    /// If the argument is `true`.
    #[inline]
    #[allow(unused_mut)]
    pub fn use_idna_2008_rules(mut self, value: bool) -> Self {
        assert!(!value, "IDNA 2008 rules are no longer supported");
        self
    }

    /// Compute the deny list
    fn deny_list(&self) -> AsciiDenyList {
        if self.use_std3_ascii_rules {
            AsciiDenyList::STD3
        } else {
            AsciiDenyList::EMPTY
        }
    }

    /// Compute the hyphen mode
    fn hyphens(&self) -> Hyphens {
        if self.check_hyphens {
            Hyphens::CheckFirstLast
        } else {
            Hyphens::Allow
        }
    }

    /// [UTS 46 ToASCII](http://www.unicode.org/reports/tr46/#ToASCII)
    pub fn to_ascii(self, domain: &str) -> Result<String, Errors> {
        let mut result = String::with_capacity(domain.len());
        let mut codec = Idna::new(self);
        codec.to_ascii(domain, &mut result).map(|()| result)
    }

    /// [UTS 46 ToUnicode](http://www.unicode.org/reports/tr46/#ToUnicode)
    pub fn to_unicode(self, domain: &str) -> (String, Result<(), Errors>) {
        let mut codec = Idna::new(self);
        let mut out = String::with_capacity(domain.len());
        let result = codec.to_unicode(domain, &mut out);
        (out, result)
    }
}
