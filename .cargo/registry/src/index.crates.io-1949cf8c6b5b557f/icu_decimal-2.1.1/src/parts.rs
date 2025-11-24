// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Parts of a formatted decimal.
//!
//! # Examples
//!
//! ```
//! use icu::decimal::parts;
//! use icu::decimal::DecimalFormatter;
//! use icu::locale::locale;
//! use writeable::assert_writeable_parts_eq;
//!
//! let formatter = DecimalFormatter::try_new(
//!     locale!("en").into(),
//!     Default::default(),
//! )
//! .unwrap();
//!
//! let decimal = "-987654.321".parse().unwrap();
//!
//! // Missing data is filled in on a best-effort basis, and an error is signaled.
//! assert_writeable_parts_eq!(
//!     formatter.format(&decimal),
//!     "-987,654.321",
//!     [
//!         (0, 1, parts::MINUS_SIGN),
//!         (1, 8, parts::INTEGER),
//!         (4, 5, parts::GROUP),
//!         (8, 9, parts::DECIMAL),
//!         (9, 12, parts::FRACTION),
//!     ]
//! );
//! ```

use writeable::Part;

/// A [`Part`] used by [`FormattedDecimal`](super::FormattedDecimal).
pub const PLUS_SIGN: Part = Part {
    category: "decimal",
    value: "plusSign",
};

/// A [`Part`] used by [`FormattedDecimal`](super::FormattedDecimal).
pub const MINUS_SIGN: Part = Part {
    category: "decimal",
    value: "minusSign",
};

/// A [`Part`] used by [`FormattedDecimal`](super::FormattedDecimal).
pub const INTEGER: Part = Part {
    category: "decimal",
    value: "integer",
};

/// A [`Part`] used by [`FormattedDecimal`](super::FormattedDecimal).
pub const FRACTION: Part = Part {
    category: "decimal",
    value: "fraction",
};

/// A [`Part`] used by [`FormattedDecimal`](super::FormattedDecimal).
pub const GROUP: Part = Part {
    category: "decimal",
    value: "group",
};

/// A [`Part`] used by [`FormattedDecimal`](super::FormattedDecimal).
pub const DECIMAL: Part = Part {
    category: "decimal",
    value: "decimal",
};
