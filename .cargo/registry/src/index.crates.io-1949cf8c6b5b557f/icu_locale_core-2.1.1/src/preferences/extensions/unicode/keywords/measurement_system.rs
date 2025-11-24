// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Unicode Measurement System Identifier defines a preferred measurement system.
    ///
    /// Specifying "ms" in a locale identifier overrides the default value specified by supplemental measurement system data for the region
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeMeasurementSystemIdentifier).
    MeasurementSystem {
        /// Metric System
        ("metric" => Metric),
        /// US System of measurement: feet, pints, etc.; pints are 16oz
        ("ussystem" => USSystem),
        /// UK System of measurement: feet, pints, etc.; pints are 20oz
        ("uksystem" => UKSystem)
}, "ms");
