//! [![github]](https://github.com/dtolnay/semver)&ensp;[![crates-io]](https://crates.io/crates/semver)&ensp;[![docs-rs]](https://docs.rs/semver)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! A parser and evaluator for Cargo's flavor of Semantic Versioning.
//!
//! Semantic Versioning (see <https://semver.org>) is a guideline for how
//! version numbers are assigned and incremented. It is widely followed within
//! the Cargo/crates.io ecosystem for Rust.
//!
//! <br>
//!
//! # Example
//!
//! ```
//! use semver::{BuildMetadata, Prerelease, Version, VersionReq};
//!
//! fn main() {
//!     let req = VersionReq::parse(">=1.2.3, <1.8.0").unwrap();
//!
//!     // Check whether this requirement matches version 1.2.3-alpha.1 (no)
//!     let version = Version {
//!         major: 1,
//!         minor: 2,
//!         patch: 3,
//!         pre: Prerelease::new("alpha.1").unwrap(),
//!         build: BuildMetadata::EMPTY,
//!     };
//!     assert!(!req.matches(&version));
//!
//!     // Check whether it matches 1.3.0 (yes it does)
//!     let version = Version::parse("1.3.0").unwrap();
//!     assert!(req.matches(&version));
//! }
//! ```
//!
//! <br><br>
//!
//! # Scope of this crate
//!
//! Besides Cargo, several other package ecosystems and package managers for
//! other languages also use SemVer:&ensp;RubyGems/Bundler for Ruby, npm for
//! JavaScript, Composer for PHP, CocoaPods for Objective-C...
//!
//! The `semver` crate is specifically intended to implement Cargo's
//! interpretation of Semantic Versioning.
//!
//! Where the various tools differ in their interpretation or implementation of
//! the spec, this crate follows the implementation choices made by Cargo. If
//! you are operating on version numbers from some other package ecosystem, you
//! will want to use a different semver library which is appropriate to that
//! ecosystem.
//!
//! The extent of Cargo's SemVer support is documented in the *[Specifying
//! Dependencies]* chapter of the Cargo reference.
//!
//! [Specifying Dependencies]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html

#![doc(html_root_url = "https://docs.rs/semver/1.0.27")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::checked_conversions,
    clippy::doc_markdown,
    clippy::incompatible_msrv,
    clippy::items_after_statements,
    clippy::manual_map,
    clippy::manual_range_contains,
    clippy::match_bool,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::needless_doctest_main,
    clippy::ptr_as_ptr,
    clippy::redundant_else,
    clippy::semicolon_if_nothing_returned, // https://github.com/rust-lang/rust-clippy/issues/7324
    clippy::similar_names,
    clippy::uninlined_format_args,
    clippy::unnested_or_patterns,
    clippy::unseparated_literal_suffix,
    clippy::wildcard_imports
)]

extern crate alloc;

mod display;
mod error;
mod eval;
mod identifier;
mod impls;
mod parse;

#[cfg(feature = "serde")]
mod serde;

use crate::identifier::Identifier;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::str::FromStr;

pub use crate::parse::Error;

/// **SemVer version** as defined by <https://semver.org>.
///
/// # Syntax
///
/// - The major, minor, and patch numbers may be any integer 0 through u64::MAX.
///   When representing a SemVer version as a string, each number is written as
///   a base 10 integer. For example, `1.0.119`.
///
/// - Leading zeros are forbidden in those positions. For example `1.01.00` is
///   invalid as a SemVer version.
///
/// - The pre-release identifier, if present, must conform to the syntax
///   documented for [`Prerelease`].
///
/// - The build metadata, if present, must conform to the syntax documented for
///   [`BuildMetadata`].
///
/// - Whitespace is not allowed anywhere in the version.
///
/// # Total ordering
///
/// Given any two SemVer versions, one is less than, greater than, or equal to
/// the other. Versions may be compared against one another using Rust's usual
/// comparison operators.
///
/// - The major, minor, and patch number are compared numerically from left to
///   right, lexicographically ordered as a 3-tuple of integers. So for example
///   version `1.5.0` is less than version `1.19.0`, despite the fact that
///   "1.19.0" &lt; "1.5.0" as ASCIIbetically compared strings and 1.19 &lt; 1.5
///   as real numbers.
///
/// - When major, minor, and patch are equal, a pre-release version is
///   considered less than the ordinary release:&ensp;version `1.0.0-alpha.1` is
///   less than version `1.0.0`.
///
/// - Two pre-releases of the same major, minor, patch are compared by
///   lexicographic ordering of dot-separated components of the pre-release
///   string.
///
///   - Identifiers consisting of only digits are compared
///     numerically:&ensp;`1.0.0-pre.8` is less than `1.0.0-pre.12`.
///
///   - Identifiers that contain a letter or hyphen are compared in ASCII sort
///     order:&ensp;`1.0.0-pre12` is less than `1.0.0-pre8`.
///
///   - Any numeric identifier is always less than any non-numeric
///     identifier:&ensp;`1.0.0-pre.1` is less than `1.0.0-pre.x`.
///
/// Example:&ensp;`1.0.0-alpha`&ensp;&lt;&ensp;`1.0.0-alpha.1`&ensp;&lt;&ensp;`1.0.0-alpha.beta`&ensp;&lt;&ensp;`1.0.0-beta`&ensp;&lt;&ensp;`1.0.0-beta.2`&ensp;&lt;&ensp;`1.0.0-beta.11`&ensp;&lt;&ensp;`1.0.0-rc.1`&ensp;&lt;&ensp;`1.0.0`
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Prerelease,
    pub build: BuildMetadata,
}

/// **SemVer version requirement** describing the intersection of some version
/// comparators, such as `>=1.2.3, <1.8`.
///
/// # Syntax
///
/// - Either `*` (meaning "any"), or one or more comma-separated comparators.
///
/// - A [`Comparator`] is an operator ([`Op`]) and a partial version, separated
///   by optional whitespace. For example `>=1.0.0` or `>=1.0`.
///
/// - Build metadata is syntactically permitted on the partial versions, but is
///   completely ignored, as it's never relevant to whether any comparator
///   matches a particular version.
///
/// - Whitespace is permitted around commas and around operators. Whitespace is
///   not permitted within a partial version, i.e. anywhere between the major
///   version number and its minor, patch, pre-release, or build metadata.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct VersionReq {
    pub comparators: Vec<Comparator>,
}

/// A pair of comparison operator and partial version, such as `>=1.2`. Forms
/// one piece of a VersionReq.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Comparator {
    pub op: Op,
    pub major: u64,
    pub minor: Option<u64>,
    /// Patch is only allowed if minor is Some.
    pub patch: Option<u64>,
    /// Non-empty pre-release is only allowed if patch is Some.
    pub pre: Prerelease,
}

/// SemVer comparison operator: `=`, `>`, `>=`, `<`, `<=`, `~`, `^`, `*`.
///
/// # Op::Exact
/// - &ensp;**`=I.J.K`**&emsp;&mdash;&emsp;exactly the version I.J.K
/// - &ensp;**`=I.J`**&emsp;&mdash;&emsp;equivalent to `>=I.J.0, <I.(J+1).0`
/// - &ensp;**`=I`**&emsp;&mdash;&emsp;equivalent to `>=I.0.0, <(I+1).0.0`
///
/// # Op::Greater
/// - &ensp;**`>I.J.K`**
/// - &ensp;**`>I.J`**&emsp;&mdash;&emsp;equivalent to `>=I.(J+1).0`
/// - &ensp;**`>I`**&emsp;&mdash;&emsp;equivalent to `>=(I+1).0.0`
///
/// # Op::GreaterEq
/// - &ensp;**`>=I.J.K`**
/// - &ensp;**`>=I.J`**&emsp;&mdash;&emsp;equivalent to `>=I.J.0`
/// - &ensp;**`>=I`**&emsp;&mdash;&emsp;equivalent to `>=I.0.0`
///
/// # Op::Less
/// - &ensp;**`<I.J.K`**
/// - &ensp;**`<I.J`**&emsp;&mdash;&emsp;equivalent to `<I.J.0`
/// - &ensp;**`<I`**&emsp;&mdash;&emsp;equivalent to `<I.0.0`
///
/// # Op::LessEq
/// - &ensp;**`<=I.J.K`**
/// - &ensp;**`<=I.J`**&emsp;&mdash;&emsp;equivalent to `<I.(J+1).0`
/// - &ensp;**`<=I`**&emsp;&mdash;&emsp;equivalent to `<(I+1).0.0`
///
/// # Op::Tilde&emsp;("patch" updates)
/// *Tilde requirements allow the **patch** part of the semver version (the third number) to increase.*
/// - &ensp;**`~I.J.K`**&emsp;&mdash;&emsp;equivalent to `>=I.J.K, <I.(J+1).0`
/// - &ensp;**`~I.J`**&emsp;&mdash;&emsp;equivalent to `=I.J`
/// - &ensp;**`~I`**&emsp;&mdash;&emsp;equivalent to `=I`
///
/// # Op::Caret&emsp;("compatible" updates)
/// *Caret requirements allow parts that are **right of the first nonzero** part of the semver version to increase.*
/// - &ensp;**`^I.J.K`**&ensp;(for I\>0)&emsp;&mdash;&emsp;equivalent to `>=I.J.K, <(I+1).0.0`
/// - &ensp;**`^0.J.K`**&ensp;(for J\>0)&emsp;&mdash;&emsp;equivalent to `>=0.J.K, <0.(J+1).0`
/// - &ensp;**`^0.0.K`**&emsp;&mdash;&emsp;equivalent to `=0.0.K`
/// - &ensp;**`^I.J`**&ensp;(for I\>0 or J\>0)&emsp;&mdash;&emsp;equivalent to `^I.J.0`
/// - &ensp;**`^0.0`**&emsp;&mdash;&emsp;equivalent to `=0.0`
/// - &ensp;**`^I`**&emsp;&mdash;&emsp;equivalent to `=I`
///
/// # Op::Wildcard
/// - &ensp;**`I.J.*`**&emsp;&mdash;&emsp;equivalent to `=I.J`
/// - &ensp;**`I.*`**&ensp;or&ensp;**`I.*.*`**&emsp;&mdash;&emsp;equivalent to `=I`
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum Op {
    Exact,
    Greater,
    GreaterEq,
    Less,
    LessEq,
    Tilde,
    Caret,
    Wildcard,
}

/// Optional pre-release identifier on a version string. This comes after `-` in
/// a SemVer version, like `1.0.0-alpha.1`
///
/// # Examples
///
/// Some real world pre-release idioms drawn from crates.io:
///
/// - **[mio]** <code>0.7.0-<b>alpha.1</b></code> &mdash; the most common style
///   for numbering pre-releases.
///
/// - **[pest]** <code>1.0.0-<b>beta.8</b></code>,&ensp;<code>1.0.0-<b>rc.0</b></code>
///   &mdash; this crate makes a distinction between betas and release
///   candidates.
///
/// - **[sassers]** <code>0.11.0-<b>shitshow</b></code> &mdash; ???.
///
/// - **[atomic-utils]** <code>0.0.0-<b>reserved</b></code> &mdash; a squatted
///   crate name.
///
/// [mio]: https://crates.io/crates/mio
/// [pest]: https://crates.io/crates/pest
/// [atomic-utils]: https://crates.io/crates/atomic-utils
/// [sassers]: https://crates.io/crates/sassers
///
/// *Tip:* Be aware that if you are planning to number your own pre-releases,
/// you should prefer to separate the numeric part from any non-numeric
/// identifiers by using a dot in between. That is, prefer pre-releases
/// `alpha.1`, `alpha.2`, etc rather than `alpha1`, `alpha2` etc. The SemVer
/// spec's rule for pre-release precedence has special treatment of numeric
/// components in the pre-release string, but only if there are no non-digit
/// characters in the same dot-separated component. So you'd have `alpha.2` &lt;
/// `alpha.11` as intended, but `alpha11` &lt; `alpha2`.
///
/// # Syntax
///
/// Pre-release strings are a series of dot separated identifiers immediately
/// following the patch version. Identifiers must comprise only ASCII
/// alphanumerics and hyphens: `0-9`, `A-Z`, `a-z`, `-`. Identifiers must not be
/// empty. Numeric identifiers must not include leading zeros.
///
/// # Total ordering
///
/// Pre-releases have a total order defined by the SemVer spec. It uses
/// lexicographic ordering of dot-separated components. Identifiers consisting
/// of only digits are compared numerically. Otherwise, identifiers are compared
/// in ASCII sort order. Any numeric identifier is always less than any
/// non-numeric identifier.
///
/// Example:&ensp;`alpha`&ensp;&lt;&ensp;`alpha.85`&ensp;&lt;&ensp;`alpha.90`&ensp;&lt;&ensp;`alpha.200`&ensp;&lt;&ensp;`alpha.0a`&ensp;&lt;&ensp;`alpha.1a0`&ensp;&lt;&ensp;`alpha.a`&ensp;&lt;&ensp;`beta`
#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct Prerelease {
    identifier: Identifier,
}

/// Optional build metadata identifier. This comes after `+` in a SemVer
/// version, as in `0.8.1+zstd.1.5.0`.
///
/// # Examples
///
/// Some real world build metadata idioms drawn from crates.io:
///
/// - **[libgit2-sys]** <code>0.12.20+<b>1.1.0</b></code> &mdash; for this
///   crate, the build metadata indicates the version of the C libgit2 library
///   that the Rust crate is built against.
///
/// - **[mashup]** <code>0.1.13+<b>deprecated</b></code> &mdash; just the word
///   "deprecated" for a crate that has been superseded by another. Eventually
///   people will take notice of this in Cargo's build output where it lists the
///   crates being compiled.
///
/// - **[google-bigquery2]** <code>2.0.4+<b>20210327</b></code> &mdash; this
///   library is automatically generated from an official API schema, and the
///   build metadata indicates the date on which that schema was last captured.
///
/// - **[fbthrift-git]** <code>0.0.6+<b>c7fcc0e</b></code> &mdash; this crate is
///   published from snapshots of a big company monorepo. In monorepo
///   development, there is no concept of versions, and all downstream code is
///   just updated atomically in the same commit that breaking changes to a
///   library are landed. Therefore for crates.io purposes, every published
///   version must be assumed to be incompatible with the previous. The build
///   metadata provides the source control hash of the snapshotted code.
///
/// [libgit2-sys]: https://crates.io/crates/libgit2-sys
/// [mashup]: https://crates.io/crates/mashup
/// [google-bigquery2]: https://crates.io/crates/google-bigquery2
/// [fbthrift-git]: https://crates.io/crates/fbthrift-git
///
/// # Syntax
///
/// Build metadata is a series of dot separated identifiers immediately
/// following the patch or pre-release version. Identifiers must comprise only
/// ASCII alphanumerics and hyphens: `0-9`, `A-Z`, `a-z`, `-`. Identifiers must
/// not be empty. Leading zeros *are* allowed, unlike any other place in the
/// SemVer grammar.
///
/// # Total ordering
///
/// Build metadata is ignored in evaluating `VersionReq`; it plays no role in
/// whether a `Version` matches any one of the comparison operators.
///
/// However for comparing build metadatas among one another, they do have a
/// total order which is determined by lexicographic ordering of dot-separated
/// components. Identifiers consisting of only digits are compared numerically.
/// Otherwise, identifiers are compared in ASCII sort order. Any numeric
/// identifier is always less than any non-numeric identifier.
///
/// Example:&ensp;`demo`&ensp;&lt;&ensp;`demo.85`&ensp;&lt;&ensp;`demo.90`&ensp;&lt;&ensp;`demo.090`&ensp;&lt;&ensp;`demo.200`&ensp;&lt;&ensp;`demo.1a0`&ensp;&lt;&ensp;`demo.a`&ensp;&lt;&ensp;`memo`
#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct BuildMetadata {
    identifier: Identifier,
}

impl Version {
    /// Create `Version` with an empty pre-release and build metadata.
    ///
    /// Equivalent to:
    ///
    /// ```
    /// # use semver::{BuildMetadata, Prerelease, Version};
    /// #
    /// # const fn new(major: u64, minor: u64, patch: u64) -> Version {
    /// Version {
    ///     major,
    ///     minor,
    ///     patch,
    ///     pre: Prerelease::EMPTY,
    ///     build: BuildMetadata::EMPTY,
    /// }
    /// # }
    /// ```
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Version {
            major,
            minor,
            patch,
            pre: Prerelease::EMPTY,
            build: BuildMetadata::EMPTY,
        }
    }

    /// Create `Version` by parsing from string representation.
    ///
    /// # Errors
    ///
    /// Possible reasons for the parse to fail include:
    ///
    /// - `1.0` &mdash; too few numeric components. A SemVer version must have
    ///   exactly three. If you are looking at something that has fewer than
    ///   three numbers in it, it's possible it is a `VersionReq` instead (with
    ///   an implicit default `^` comparison operator).
    ///
    /// - `1.0.01` &mdash; a numeric component has a leading zero.
    ///
    /// - `1.0.unknown` &mdash; unexpected character in one of the components.
    ///
    /// - `1.0.0-` or `1.0.0+` &mdash; the pre-release or build metadata are
    ///   indicated present but empty.
    ///
    /// - `1.0.0-alpha_123` &mdash; pre-release or build metadata have something
    ///   outside the allowed characters, which are `0-9`, `A-Z`, `a-z`, `-`,
    ///   and `.` (dot).
    ///
    /// - `23456789999999999999.0.0` &mdash; overflow of a u64.
    pub fn parse(text: &str) -> Result<Self, Error> {
        Version::from_str(text)
    }

    /// Compare the major, minor, patch, and pre-release value of two versions,
    /// disregarding build metadata. Versions that differ only in build metadata
    /// are considered equal. This comparison is what the SemVer spec refers to
    /// as "precedence".
    ///
    /// # Example
    ///
    /// ```
    /// use semver::Version;
    ///
    /// let mut versions = [
    ///     "1.20.0+c144a98".parse::<Version>().unwrap(),
    ///     "1.20.0".parse().unwrap(),
    ///     "1.0.0".parse().unwrap(),
    ///     "1.0.0-alpha".parse().unwrap(),
    ///     "1.20.0+bc17664".parse().unwrap(),
    /// ];
    ///
    /// // This is a stable sort, so it preserves the relative order of equal
    /// // elements. The three 1.20.0 versions differ only in build metadata so
    /// // they are not reordered relative to one another.
    /// versions.sort_by(Version::cmp_precedence);
    /// assert_eq!(versions, [
    ///     "1.0.0-alpha".parse().unwrap(),
    ///     "1.0.0".parse().unwrap(),
    ///     "1.20.0+c144a98".parse().unwrap(),
    ///     "1.20.0".parse().unwrap(),
    ///     "1.20.0+bc17664".parse().unwrap(),
    /// ]);
    ///
    /// // Totally order the versions, including comparing the build metadata.
    /// versions.sort();
    /// assert_eq!(versions, [
    ///     "1.0.0-alpha".parse().unwrap(),
    ///     "1.0.0".parse().unwrap(),
    ///     "1.20.0".parse().unwrap(),
    ///     "1.20.0+bc17664".parse().unwrap(),
    ///     "1.20.0+c144a98".parse().unwrap(),
    /// ]);
    /// ```
    pub fn cmp_precedence(&self, other: &Self) -> Ordering {
        Ord::cmp(
            &(self.major, self.minor, self.patch, &self.pre),
            &(other.major, other.minor, other.patch, &other.pre),
        )
    }
}

impl VersionReq {
    /// A `VersionReq` with no constraint on the version numbers it matches.
    /// Equivalent to `VersionReq::parse("*").unwrap()`.
    ///
    /// In terms of comparators this is equivalent to `>=0.0.0`.
    ///
    /// Counterintuitively a `*` VersionReq does not match every possible
    /// version number. In particular, in order for *any* `VersionReq` to match
    /// a pre-release version, the `VersionReq` must contain at least one
    /// `Comparator` that has an explicit major, minor, and patch version
    /// identical to the pre-release being matched, and that has a nonempty
    /// pre-release component. Since `*` is not written with an explicit major,
    /// minor, and patch version, and does not contain a nonempty pre-release
    /// component, it does not match any pre-release versions.
    pub const STAR: Self = VersionReq {
        comparators: Vec::new(),
    };

    /// Create `VersionReq` by parsing from string representation.
    ///
    /// # Errors
    ///
    /// Possible reasons for the parse to fail include:
    ///
    /// - `>a.b` &mdash; unexpected characters in the partial version.
    ///
    /// - `@1.0.0` &mdash; unrecognized comparison operator.
    ///
    /// - `^1.0.0, ` &mdash; unexpected end of input.
    ///
    /// - `>=1.0 <2.0` &mdash; missing comma between comparators.
    ///
    /// - `*.*` &mdash; unsupported wildcard syntax.
    pub fn parse(text: &str) -> Result<Self, Error> {
        VersionReq::from_str(text)
    }

    /// Evaluate whether the given `Version` satisfies the version requirement
    /// described by `self`.
    pub fn matches(&self, version: &Version) -> bool {
        eval::matches_req(self, version)
    }
}

/// The default VersionReq is the same as [`VersionReq::STAR`].
impl Default for VersionReq {
    fn default() -> Self {
        VersionReq::STAR
    }
}

impl Comparator {
    pub fn parse(text: &str) -> Result<Self, Error> {
        Comparator::from_str(text)
    }

    pub fn matches(&self, version: &Version) -> bool {
        eval::matches_comparator(self, version)
    }
}

impl Prerelease {
    pub const EMPTY: Self = Prerelease {
        identifier: Identifier::empty(),
    };

    pub fn new(text: &str) -> Result<Self, Error> {
        Prerelease::from_str(text)
    }

    pub fn as_str(&self) -> &str {
        self.identifier.as_str()
    }

    pub fn is_empty(&self) -> bool {
        self.identifier.is_empty()
    }
}

impl BuildMetadata {
    pub const EMPTY: Self = BuildMetadata {
        identifier: Identifier::empty(),
    };

    pub fn new(text: &str) -> Result<Self, Error> {
        BuildMetadata::from_str(text)
    }

    pub fn as_str(&self) -> &str {
        self.identifier.as_str()
    }

    pub fn is_empty(&self) -> bool {
        self.identifier.is_empty()
    }
}
