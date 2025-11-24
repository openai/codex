// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Unicode Emoji Presentation Style Identifier
    ///
    /// It specifies a request for the preferred emoji
    /// presentation style. This can be used as part of the value for an HTML lang attribute,
    /// for example `<html lang="sr-Latn-u-em-emoji">`.
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeEmojiPresentationStyleIdentifier).
    [Default]
    EmojiPresentationStyle {
        /// Use an emoji presentation for emoji characters if possible
        ("emoji" => Emoji),
        /// Use a text presentation for emoji characters if possible
        ("text" => Text),
        /// Use the default presentation for emoji characters as specified in UTR #51 Presentation Style
        [default]
        ("default" => Default)
}, "em");
