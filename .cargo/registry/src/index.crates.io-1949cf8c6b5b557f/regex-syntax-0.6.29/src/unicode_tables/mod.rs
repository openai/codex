#[cfg(feature = "unicode-age")]
pub mod age;

#[cfg(feature = "unicode-case")]
pub mod case_folding_simple;

#[cfg(feature = "unicode-gencat")]
pub mod general_category;

#[cfg(feature = "unicode-segment")]
pub mod grapheme_cluster_break;

#[cfg(all(feature = "unicode-perl", not(feature = "unicode-gencat")))]
#[allow(dead_code)]
pub mod perl_decimal;

#[cfg(all(feature = "unicode-perl", not(feature = "unicode-bool")))]
#[allow(dead_code)]
pub mod perl_space;

#[cfg(feature = "unicode-perl")]
pub mod perl_word;

#[cfg(feature = "unicode-bool")]
pub mod property_bool;

#[cfg(any(
    feature = "unicode-age",
    feature = "unicode-bool",
    feature = "unicode-gencat",
    feature = "unicode-perl",
    feature = "unicode-script",
    feature = "unicode-segment",
))]
pub mod property_names;

#[cfg(any(
    feature = "unicode-age",
    feature = "unicode-bool",
    feature = "unicode-gencat",
    feature = "unicode-perl",
    feature = "unicode-script",
    feature = "unicode-segment",
))]
pub mod property_values;

#[cfg(feature = "unicode-script")]
pub mod script;

#[cfg(feature = "unicode-script")]
pub mod script_extension;

#[cfg(feature = "unicode-segment")]
pub mod sentence_break;

#[cfg(feature = "unicode-segment")]
pub mod word_break;
