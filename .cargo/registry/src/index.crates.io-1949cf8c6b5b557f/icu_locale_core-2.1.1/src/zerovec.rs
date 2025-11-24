// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Documentation on zero-copy deserialization of locale types.
//!
//! [`Locale`] and [`LanguageIdentifier`] are highly structured types that cannot be directly
//! stored in a zero-copy data structure, such as those provided by the [`zerovec`](crate::zerovec) module.
//! This page explains how to indirectly store these types in a [`zerovec`](crate::zerovec).
//!
//! There are two main use cases, which have different solutions:
//!
//! 1. **Lookup:** You need to locate a locale in a zero-copy vector, such as when querying a map.
//! 2. **Obtain:** You have a locale stored in a zero-copy vector, and you need to obtain a proper
//!    [`Locale`] or [`LanguageIdentifier`] for use elsewhere in your program.
//!
//! # Lookup
//!
//! To perform lookup, store the stringified locale in a canonical BCP-47 form as a byte array,
//! and then use [`Locale::strict_cmp()`] to perform an efficient, zero-allocation lookup.
//!
//! To produce more human-readable serialized output, you can use `PotentialUtf8`.
//!
//! ```
//! use icu::locale::Locale;
//! use potential_utf::PotentialUtf8;
//! use zerovec::ZeroMap;
//!
//! // ZeroMap from locales to integers
//! let data: &[(&PotentialUtf8, u32)] = &[
//!     ("de-DE-u-hc-h12".into(), 5),
//!     ("en-US-u-ca-buddhist".into(), 10),
//!     ("my-MM".into(), 15),
//!     ("sr-Cyrl-ME".into(), 20),
//!     ("zh-TW".into(), 25),
//! ];
//! let zm: ZeroMap<PotentialUtf8, u32> = data.iter().copied().collect();
//!
//! // Get the value associated with a locale
//! let loc: Locale = "en-US-u-ca-buddhist".parse().unwrap();
//! let value = zm.get_copied_by(|uvstr| loc.strict_cmp(uvstr).reverse());
//! assert_eq!(value, Some(10));
//! ```
//!
//! # Obtain
//!
//! Obtaining a [`Locale`] or [`LanguageIdentifier`] is not generally a zero-copy operation, since
//! both of these types may require memory allocation. If possible, architect your code such that
//! you do not need to obtain a structured type.
//!
//! If you need the structured type, such as if you need to manipulate it in some way, there are two
//! options: storing subtags, and storing a string for parsing.
//!
//! ## Storing Subtags
//!
//! If the data being stored only contains a limited number of subtags, you can store them as a
//! tuple, and then construct the [`LanguageIdentifier`] externally.
//!
//! ```
//! use icu::locale::subtags::{Language, Region, Script};
//! use icu::locale::LanguageIdentifier;
//! use icu::locale::{
//!     langid,
//!     subtags::{language, region, script},
//! };
//! use zerovec::ZeroMap;
//!
//! // ZeroMap from integer to LSR (language-script-region)
//! let zm: ZeroMap<u32, (Language, Option<Script>, Option<Region>)> = [
//!     (5, (language!("de"), None, Some(region!("DE")))),
//!     (10, (language!("en"), None, Some(region!("US")))),
//!     (15, (language!("my"), None, Some(region!("MM")))),
//!     (
//!         20,
//!         (language!("sr"), Some(script!("Cyrl")), Some(region!("ME"))),
//!     ),
//!     (25, (language!("zh"), None, Some(region!("TW")))),
//! ]
//! .into_iter()
//! .collect();
//!
//! // Construct a LanguageIdentifier from a tuple entry
//! let lid: LanguageIdentifier =
//!     zm.get_copied(&25).expect("element is present").into();
//!
//! assert_eq!(lid, langid!("zh-TW"));
//! ```
//!
//! ## Storing Strings
//!
//! If it is necessary to store and obtain an arbitrary locale, it is currently recommended to
//! store a BCP-47 string and parse it when needed.
//!
//! Since the string is stored in an unparsed state, it is not safe to `unwrap` the result from
//! `Locale::try_from_utf8()`. See [icu4x#831](https://github.com/unicode-org/icu4x/issues/831)
//! for a discussion on potential data models that could ensure that the locale is valid during
//! deserialization.
//!
//! As above, to produce more human-readable serialized output, you can use `PotentialUtf8`.
//!
//! ```
//! use icu::locale::langid;
//! use icu::locale::Locale;
//! use potential_utf::PotentialUtf8;
//! use zerovec::ZeroMap;
//!
//! // ZeroMap from integer to locale string
//! let data: &[(u32, &PotentialUtf8)] = &[
//!     (5, "de-DE-u-hc-h12".into()),
//!     (10, "en-US-u-ca-buddhist".into()),
//!     (15, "my-MM".into()),
//!     (20, "sr-Cyrl-ME".into()),
//!     (25, "zh-TW".into()),
//!     (30, "INVALID".into()),
//! ];
//! let zm: ZeroMap<u32, PotentialUtf8> = data.iter().copied().collect();
//!
//! // Construct a Locale by parsing the string.
//! let value = zm.get(&25).expect("element is present");
//! let loc = Locale::try_from_utf8(value);
//! assert_eq!(loc, Ok(langid!("zh-TW").into()));
//!
//! // Invalid entries are fallible
//! let err_value = zm.get(&30).expect("element is present");
//! let err_loc = Locale::try_from_utf8(err_value);
//! assert!(err_loc.is_err());
//! ```
//!
//! [`Locale`]: crate::Locale
//! [`Locale::strict_cmp()`]: crate::Locale::strict_cmp()
//! [`LanguageIdentifier`]: crate::LanguageIdentifier
