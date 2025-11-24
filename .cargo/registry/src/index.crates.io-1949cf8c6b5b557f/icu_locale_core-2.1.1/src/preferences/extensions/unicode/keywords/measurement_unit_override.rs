// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Measurement Unit Preference Override defines an override for measurement unit preference.
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#MeasurementUnitPreferenceOverride).
    MeasurementUnitOverride {
        /// Celsius as temperature unit
        ("celsius" => Celsius),
        /// Kelvin as temperature unit
        ("kelvin" => Kelvin),
        /// Fahrenheit as temperature unit
        ("fahrenhe" => Fahrenheit),
}, "mu");
