// This cfg should match the one in src/util/look.rs that uses perl_word.
#[cfg(all(
    // We have to explicitly want to support Unicode word boundaries.
    feature = "unicode-word-boundary",
    not(all(
        // If we don't have regex-syntax at all, then we definitely need to
        // bring our own \w data table.
        feature = "syntax",
        // If unicode-perl is enabled, then regex-syntax/unicode-perl is
        // also enabled, which in turn means we can use regex-syntax's
        // is_word_character routine (and thus use its data tables). But if
        // unicode-perl is not enabled, even if syntax is, then we need to
        // bring our own.
        feature = "unicode-perl",
    )),
))]
pub(crate) mod perl_word;
