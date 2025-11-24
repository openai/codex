// Copyright 2012-2025 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Determine displayed width of `char` and `str` types according to
//! [Unicode Standard Annex #11](http://www.unicode.org/reports/tr11/)
//! and other portions of the Unicode standard.
//! See the [Rules for determining width](#rules-for-determining-width) section
//! for the exact rules.
//!
//! This crate is `#![no_std]`.
//!
//! ```rust
//! use unicode_width::UnicodeWidthStr;
//!
//! let teststr = "Ｈｅｌｌｏ, ｗｏｒｌｄ!";
//! let width = UnicodeWidthStr::width(teststr);
//! println!("{}", teststr);
//! println!("The above string is {} columns wide.", width);
//! ```
//!
//! # `"cjk"` feature flag
//!
//! This crate has one Cargo feature flag, `"cjk"`
//! (enabled by default).
//! It enables the [`UnicodeWidthChar::width_cjk`]
//! and [`UnicodeWidthStr::width_cjk`],
//! which perform an alternate width calculation
//! more suited to CJK contexts. The flag also unseals the
//! [`UnicodeWidthChar`] and [`UnicodeWidthStr`] traits.
//!
//! Disabling the flag (with `no_default_features` in `Cargo.toml`)
//! will reduce the amount of static data needed by the crate.
//!
//! ```rust
//! use unicode_width::UnicodeWidthStr;
//!
//! let teststr = "“𘀀”";
//! assert_eq!(teststr.width(), 4);
//!
//! #[cfg(feature = "cjk")]
//! assert_eq!(teststr.width_cjk(), 6);
//! ```
//!
//! # Rules for determining width
//!
//! This crate currently uses the following rules to determine the width of a
//! character or string, in order of decreasing precedence. These may be tweaked in the future.
//!
//! 1. In the following cases, the width of a string differs from the sum of the widths of its constituent characters:
//!    - The sequence `"\r\n"` has width 1.
//!    - Emoji-specific ligatures:
//!      - Well-formed, fully-qualified [emoji ZWJ sequences] have width 2.
//!      - [Emoji modifier sequences] have width 2.
//!      - [Emoji presentation sequences] have width 2.
//!      - Outside of an East Asian context, [text presentation sequences] have width 1 if their base character:
//!        - Has the [`Emoji_Presentation`] property, and
//!        - Is not in the [Enclosed Ideographic Supplement] block.
//!    - [`'\u{2018}'`, `'\u{2019}'`, `'\u{201C}'`, and `'\u{201D}'`][General Punctuation] always have width 1 when followed by '\u{FE00}',
//!      and width 2 when followed by '\u{FE01}'.
//!    - Script-specific ligatures:
//!      - For all the following ligatures, the insertion of any number of [default-ignorable][`Default_Ignorable_Code_Point`]
//!        [combining marks] anywhere in the sequence will not change the total width. In addition, for all non-Arabic
//!        ligatures, the insertion of any number of [`'\u{200D}'` ZERO WIDTH JOINER](https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-23/#G23126)s
//!        will not affect the width.
//!      - **[Arabic]**: A character sequence consisting of one character with [`Joining_Group`]`=Lam`,
//!        followed by any number of characters with [`Joining_Type`]`=Transparent`, followed by one character
//!        with [`Joining_Group`]`=Alef`, has total width 1. For example: `لا`‎, `لآ`‎, `ڸا`‎, `لٟٞأ`
//!      - **[Buginese]**: `"\u{1A15}\u{1A17}\u{200D}\u{1A10}"` (<a, -i> ya, `ᨕᨗ‍ᨐ`) has total width 1.
//!      - **[Hebrew]**: `"א\u{200D}ל"` (Alef-Lamed, `א‍ל`) has total width 1.
//!      - **[Khmer]**: Coeng signs consisting of `'\u{17D2}'` followed by a character in
//!        `'\u{1780}'..='\u{1782}' | '\u{1784}'..='\u{1787}' | '\u{1789}'..='\u{178C}' | '\u{178E}'..='\u{1793}' | '\u{1795}'..='\u{1798}' | '\u{179B}'..='\u{179D}' | '\u{17A0}' | '\u{17A2}'  | '\u{17A7}' | '\u{17AB}'..='\u{17AC}' | '\u{17AF}'`
//!        have width 0.
//!      - **[Kirat Rai]**: Any sequence canonically equivalent to `'\u{16D68}'`, `'\u{16D69}'`, or `'\u{16D6A}'` has total width 1.
//!      - **[Lisu]**: Tone letter combinations consisting of a character in the range `'\u{A4F8}'..='\u{A4FB}'`
//!        followed by a character in the range `'\u{A4FC}'..='\u{A4FD}'` have width 1. For example: `ꓹꓼ`
//!      - **[Old Turkic]**: `"\u{10C32}\u{200D}\u{10C03}"` (`𐰲‍𐰃`) has total width 1.
//!      - **[Tifinagh]**: A sequence of a Tifinagh consonant in the range `'\u{2D31}'..='\u{2D65}' | '\u{2D6F}'`, followed by either
//!        [`'\u{2D7F}'` TIFINAGH CONSONANT JOINER] or `'\u{200D}'`, followed by another Tifinangh consonant, has total width 1.
//!        For example: `ⵏ⵿ⴾ`
//!    - In an East Asian context only, `<`, `=`, or `>` have width 2 when followed by [`'\u{0338}'` COMBINING LONG SOLIDUS OVERLAY].
//!      The two characters may be separated by any number of characters whose canonical decompositions consist only of characters meeting
//!      one of the following requirements:
//!      - Has [`Canonical_Combining_Class`] greater than 1, or
//!      - Is a [default-ignorable][`Default_Ignorable_Code_Point`] [combining mark][combining marks].
//! 2. In all other cases, the width of the string equals the sum of its character widths:
//!    1. [`'\u{2D7F}'` TIFINAGH CONSONANT JOINER] has width 1 (outside of the ligatures described previously).
//!    2. [`'\u{115F}'` HANGUL CHOSEONG FILLER](https://util.unicode.org/UnicodeJsps/character.jsp?a=115F) and
//!       [`'\u{17A4}'` KHMER INDEPENDENT VOWEL QAA](https://util.unicode.org/UnicodeJsps/character.jsp?a=17A4) have width 2.
//!    3. [`'\u{17D8}'` KHMER SIGN BEYYAL](https://util.unicode.org/UnicodeJsps/character.jsp?a=17D8) has width 3.
//!    4. The following have width 0:
//!       - [Characters](https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5Cp%7BDefault_Ignorable_Code_Point%7D)
//!         with the [`Default_Ignorable_Code_Point`] property.
//!       - [Characters](https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5Cp%7BGrapheme_Extend%7D)
//!         with the [`Grapheme_Extend`] property.
//!       - [Characters](https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5Cp%7BHangul_Syllable_Type%3DV%7D%5Cp%7BHangul_Syllable_Type%3DT%7D)
//!         with a [`Hangul_Syllable_Type`] of `Vowel_Jamo` (`V`) or `Trailing_Jamo` (`T`).
//!       - The following [`Prepended_Concatenation_Mark`]s:
//!         - [`'\u{0605}'` NUMBER MARK ABOVE](https://util.unicode.org/UnicodeJsps/character.jsp?a=0605),
//!         - [`'\u{070F}'` SYRIAC ABBREVIATION MARK](https://util.unicode.org/UnicodeJsps/character.jsp?a=070F),
//!         - [`'\u{0890}'` POUND MARK ABOVE](https://util.unicode.org/UnicodeJsps/character.jsp?a=0890),
//!         - [`'\u{0891}'` PIASTRE MARK ABOVE](https://util.unicode.org/UnicodeJsps/character.jsp?a=0891), and
//!         - [`'\u{08E2}'` DISPUTED END OF AYAH](https://util.unicode.org/UnicodeJsps/character.jsp?a=08E2).
//!       - [Characters](https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5Cp%7BGrapheme_Cluster_Break%3DPrepend%7D-%5Cp%7BPrepended_Concatenation_Mark%7D)
//!         with the [`Grapheme_Extend=Prepend`] property, that are not also [`Prepended_Concatenation_Mark`]s.
//!       - [`'\u{A8FA}'` DEVANAGARI CARET](https://util.unicode.org/UnicodeJsps/character.jsp?a=A8FA).
//!    5. [Characters](https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5Cp%7BEast_Asian_Width%3DF%7D%5Cp%7BEast_Asian_Width%3DW%7D)
//!       with an [`East_Asian_Width`] of [`Fullwidth`] or [`Wide`] have width 2.
//!    6. Characters fulfilling all of the following conditions have width 2 in an East Asian context, and width 1 otherwise:
//!       - Fulfills one of the following conditions:
//!         - Has an [`East_Asian_Width`] of [`Ambiguous`], or
//!         - Has a [`Line_Break`] of [`AI`], or
//!         - Has a canonical decomposition to an [`Ambiguous`] character followed by [`'\u{0338}'` COMBINING LONG SOLIDUS OVERLAY], or
//!         - Is [`'\u{0387}'` GREEK ANO TELEIA](https://util.unicode.org/UnicodeJsps/character.jsp?a=0387); and
//!       - Does not have a [`General_Category`] of `Letter` or `Modifier_Symbol`.
//!    7. All other characters have width 1.
//!
//! [`'\u{0338}'` COMBINING LONG SOLIDUS OVERLAY]: https://util.unicode.org/UnicodeJsps/character.jsp?a=0338
//! [`'\u{2D7F}'` TIFINAGH CONSONANT JOINER]: https://util.unicode.org/UnicodeJsps/character.jsp?a=2D7F
//!
//! [`Canonical_Combining_Class`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-3/#G50313
//! [`Default_Ignorable_Code_Point`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-5/#G40095
//! [`East_Asian_Width`]: https://www.unicode.org/reports/tr11/#ED1
//! [`Emoji_Presentation`]: https://unicode.org/reports/tr51/#def_emoji_presentation
//! [`General_Category`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-4/#G124142
//! [`Grapheme_Extend=Prepend`]: https://www.unicode.org/reports/tr29/#Prepend
//! [`Grapheme_Extend`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-3/#G52443
//! [`Hangul_Syllable_Type`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-3/#G45593
//! [`Joining_Group`]: https://www.unicode.org/versions/Unicode14.0.0/ch09.pdf#G36862
//! [`Joining_Type`]: http://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-9/#G50009
//! [`Line_Break`]: https://www.unicode.org/reports/tr14/#LD5
//! [`Prepended_Concatenation_Mark`]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-23/#G37908
//! [`Script`]: https://www.unicode.org/reports/tr24/#Script
//!
//! [`Fullwidth`]: https://www.unicode.org/reports/tr11/#ED2
//! [`Wide`]: https://www.unicode.org/reports/tr11/#ED4
//! [`Ambiguous`]: https://www.unicode.org/reports/tr11/#ED6
//!
//! [`AI`]: https://www.unicode.org/reports/tr14/#AI
//!
//! [combining marks]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-3/#G30602
//!
//! [emoji ZWJ sequences]: https://www.unicode.org/reports/tr51/#def_emoji_sequence
//! [Emoji modifier sequences]: https://www.unicode.org/reports/tr51/#def_emoji_modifier_sequence
//! [Emoji presentation sequences]: https://unicode.org/reports/tr51/#def_emoji_presentation_sequence
//! [text presentation sequences]: https://unicode.org/reports/tr51/#def_text_presentation_sequence
//!
//! [General Punctuation]: https://www.unicode.org/charts/PDF/Unicode-16.0/U160-2000.pdf
//! [Enclosed Ideographic Supplement]: https://unicode.org/charts/nameslist/n_1F200.html
//!
//! [Arabic]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-9/#G7480
//! [Buginese]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-17/#G26743
//! [Hebrew]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-9/#G6528
//! [Khmer]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-16/#G64642
//! [Kirat Rai]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-13/#G746409
//! [Lisu]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-18/#G44587
//! [Old Turkic]: https://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-14/#G41975
//! [Tifinagh]: http://www.unicode.org/versions/Unicode16.0.0/core-spec/chapter-19/#G43184
//!
//!
//! ## Canonical equivalence
//!
//! Canonically equivalent strings are assigned the same width (CJK and non-CJK).

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc(
    html_logo_url = "https://unicode-rs.github.io/unicode-rs_sm.png",
    html_favicon_url = "https://unicode-rs.github.io/unicode-rs_sm.png"
)]
#![no_std]

pub use tables::UNICODE_VERSION;

mod tables;

mod private {
    pub trait Sealed {}
    #[cfg(not(feature = "cjk"))]
    impl Sealed for char {}
    #[cfg(not(feature = "cjk"))]
    impl Sealed for str {}
    #[cfg(feature = "cjk")]
    impl<T: ?Sized> Sealed for T {}
}

/// Methods for determining displayed width of Unicode characters.
pub trait UnicodeWidthChar: private::Sealed {
    /// Returns the character's displayed width in columns, or `None` if the
    /// character is a control character.
    ///
    /// This function treats characters in the Ambiguous category according
    /// to [Unicode Standard Annex #11](http://www.unicode.org/reports/tr11/)
    /// as 1 column wide. This is consistent with the recommendations for non-CJK
    /// contexts, or when the context cannot be reliably determined.
    fn width(self) -> Option<usize>;

    /// Returns the character's displayed width in columns, or `None` if the
    /// character is a control character.
    ///
    /// This function treats characters in the Ambiguous category according
    /// to [Unicode Standard Annex #11](http://www.unicode.org/reports/tr11/)
    /// as 2 columns wide. This is consistent with the recommendations for
    /// CJK contexts.
    #[cfg(feature = "cjk")]
    fn width_cjk(self) -> Option<usize>;
}

impl UnicodeWidthChar for char {
    #[inline]
    fn width(self) -> Option<usize> {
        tables::single_char_width(self)
    }

    #[cfg(feature = "cjk")]
    #[inline]
    fn width_cjk(self) -> Option<usize> {
        tables::single_char_width_cjk(self)
    }
}

/// Methods for determining displayed width of Unicode strings.
pub trait UnicodeWidthStr: private::Sealed {
    /// Returns the string's displayed width in columns.
    ///
    /// This function treats characters in the Ambiguous category according
    /// to [Unicode Standard Annex #11](http://www.unicode.org/reports/tr11/)
    /// as 1 column wide. This is consistent with the recommendations for
    /// non-CJK contexts, or when the context cannot be reliably determined.
    fn width(&self) -> usize;

    /// Returns the string's displayed width in columns.
    ///
    /// This function treats characters in the Ambiguous category according
    /// to [Unicode Standard Annex #11](http://www.unicode.org/reports/tr11/)
    /// as 2 column wide. This is consistent with the recommendations for
    /// CJK contexts.
    #[cfg(feature = "cjk")]
    fn width_cjk(&self) -> usize;
}

impl UnicodeWidthStr for str {
    #[inline]
    fn width(&self) -> usize {
        tables::str_width(self)
    }

    #[cfg(feature = "cjk")]
    #[inline]
    fn width_cjk(&self) -> usize {
        tables::str_width_cjk(self)
    }
}
