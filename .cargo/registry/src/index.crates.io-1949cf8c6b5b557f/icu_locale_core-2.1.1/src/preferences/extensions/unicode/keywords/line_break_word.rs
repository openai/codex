// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Unicode Line Break Word Identifier defines preferred line break word handling behavior corresponding to the CSS level 3 word-break option.
    ///
    /// Specifying "lw" in a locale identifier overrides the locale’s default style (which may correspond to "normal" or "keepall").
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeLineBreakWordIdentifier).
    LineBreakWordHandling {
        /// CSS lev 3 word-break=normal, normal script/language behavior for midword breaks
        ("normal" => Normal),
        /// CSS lev 3 word-break=break-all, allow midword breaks unless forbidden by lb setting
        ("breakall" => BreakAll),
        /// CSS lev 3 word-break=keep-all, prohibit midword breaks except for dictionary breaks
        ("keepall" => KeepAll),
        /// Prioritize keeping natural phrases (of multiple words) together when breaking,
        /// used in short text like title and headline
        ("phrase" => Phrase),
}, "lw");
