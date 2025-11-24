// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This module defines all available properties.
//!
//! Properties may be empty marker types and implement [`BinaryProperty`], or enumerations[^1]
//! and implement [`EnumeratedProperty`].
//!
//! [`BinaryProperty`]s are queried through a [`CodePointSetData`](crate::CodePointSetData),
//! while [`EnumeratedProperty`]s are queried through [`CodePointMapData`](crate::CodePointMapData).
//!
//! In addition, some [`EnumeratedProperty`]s also implement [`ParseableEnumeratedProperty`] or
//! [`NamedEnumeratedProperty`]. For these properties, [`PropertyParser`](crate::PropertyParser),
//! [`PropertyNamesLong`](crate::PropertyNamesLong), and [`PropertyNamesShort`](crate::PropertyNamesShort)
//! can be constructed.
//!
//! [^1]: either Rust `enum`s, or Rust `struct`s with associated constants (open enums)

pub use crate::names::{NamedEnumeratedProperty, ParseableEnumeratedProperty};

pub use crate::bidi::{BidiMirroringGlyph, BidiPairedBracketType};

/// See [`test_enumerated_property_completeness`] for usage.
/// Example input:
/// ```ignore
/// impl EastAsianWidth {
///     pub const Neutral: EastAsianWidth = EastAsianWidth(0);
///     pub const Ambiguous: EastAsianWidth = EastAsianWidth(1);
///     ...
/// }
/// ```
/// Produces `const ALL_VALUES = &[("Neutral", 0u16), ...];` by
/// explicitly casting first field of the struct to u16.
macro_rules! create_const_array {
    (
        $ ( #[$meta:meta] )*
        impl $enum_ty:ident {
            $( $(#[$const_meta:meta])* $v:vis const $i:ident: $t:ty = $e:expr; )*
        }
    ) => {
        $( #[$meta] )*
        impl $enum_ty {
            $(
                $(#[$const_meta])*
                $v const $i: $t = $e;
            )*

            /// All possible values of this enum in the Unicode version
            /// from this ICU4X release.
            pub const ALL_VALUES: &'static [$enum_ty] = &[
                $($enum_ty::$i),*
            ];
        }

        #[cfg(feature = "datagen")]
        impl databake::Bake for $enum_ty {
            fn bake(&self, env: &databake::CrateEnv) -> databake::TokenStream {
                env.insert("icu_properties");
                match *self {
                    $(
                        Self::$i => databake::quote!(icu_properties::props::$enum_ty::$i),
                    )*
                    Self(v) => databake::quote!(icu_properties::props::$enum_ty::from_icu4c_value(#v)),
                }
            }
        }


        impl From<$enum_ty> for u16  {
            fn from(other: $enum_ty) -> Self {
                other.0 as u16
            }
        }
    }
}

pub use crate::code_point_map::EnumeratedProperty;

macro_rules! make_enumerated_property {
    (
        name: $name:literal;
        short_name: $short_name:literal;
        ident: $value_ty:path;
        data_marker: $data_marker:ty;
        singleton: $singleton:ident;
        $(ule_ty: $ule_ty:ty;)?
    ) => {
        impl crate::private::Sealed for $value_ty {}

        impl EnumeratedProperty for $value_ty {
            type DataMarker = $data_marker;
            #[cfg(feature = "compiled_data")]
            const SINGLETON: &'static crate::provider::PropertyCodePointMap<'static, Self> =
                crate::provider::Baked::$singleton;
            const NAME: &'static [u8] = $name.as_bytes();
            const SHORT_NAME: &'static [u8] = $short_name.as_bytes();
        }

        $(
            impl zerovec::ule::AsULE for $value_ty {
                type ULE = $ule_ty;

                fn to_unaligned(self) -> Self::ULE {
                    self.0.to_unaligned()
                }
                fn from_unaligned(unaligned: Self::ULE) -> Self {
                    Self(zerovec::ule::AsULE::from_unaligned(unaligned))
                }
            }
        )?
    };
}

/// Enumerated property Bidi_Class
///
/// These are the categories required by the Unicode Bidirectional Algorithm.
/// For the property values, see [Bidirectional Class Values](https://unicode.org/reports/tr44/#Bidi_Class_Values).
/// For more information, see [Unicode Standard Annex #9](https://unicode.org/reports/tr41/tr41-28.html#UAX9).
///
/// # Example
///
/// ```
/// use icu::properties::{props::BidiClass, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<BidiClass>::new().get('y'),
///     BidiClass::LeftToRight
/// ); // U+0079
/// assert_eq!(
///     CodePointMapData::<BidiClass>::new().get('ع'),
///     BidiClass::ArabicLetter
/// ); // U+0639
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct BidiClass(pub(crate) u8);

impl BidiClass {
    /// Returns an ICU4C `UBidiClass` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `UBidiClass` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(non_upper_case_globals)]
impl BidiClass {
    /// (`L`) any strong left-to-right character
    pub const LeftToRight: BidiClass = BidiClass(0);
    /// (`R`) any strong right-to-left (non-Arabic-type) character
    pub const RightToLeft: BidiClass = BidiClass(1);
    /// (`EN`) any ASCII digit or Eastern Arabic-Indic digit
    pub const EuropeanNumber: BidiClass = BidiClass(2);
    /// (`ES`) plus and minus signs
    pub const EuropeanSeparator: BidiClass = BidiClass(3);
    /// (`ET`) a terminator in a numeric format context, includes currency signs
    pub const EuropeanTerminator: BidiClass = BidiClass(4);
    /// (`AN`) any Arabic-Indic digit
    pub const ArabicNumber: BidiClass = BidiClass(5);
    /// (`CS`) commas, colons, and slashes
    pub const CommonSeparator: BidiClass = BidiClass(6);
    /// (`B`) various newline characters
    pub const ParagraphSeparator: BidiClass = BidiClass(7);
    /// (`S`) various segment-related control codes
    pub const SegmentSeparator: BidiClass = BidiClass(8);
    /// (`WS`) spaces
    pub const WhiteSpace: BidiClass = BidiClass(9);
    /// (`ON`) most other symbols and punctuation marks
    pub const OtherNeutral: BidiClass = BidiClass(10);
    /// (`LRE`) U+202A: the LR embedding control
    pub const LeftToRightEmbedding: BidiClass = BidiClass(11);
    /// (`LRO`) U+202D: the LR override control
    pub const LeftToRightOverride: BidiClass = BidiClass(12);
    /// (`AL`) any strong right-to-left (Arabic-type) character
    pub const ArabicLetter: BidiClass = BidiClass(13);
    /// (`RLE`) U+202B: the RL embedding control
    pub const RightToLeftEmbedding: BidiClass = BidiClass(14);
    /// (`RLO`) U+202E: the RL override control
    pub const RightToLeftOverride: BidiClass = BidiClass(15);
    /// (`PDF`) U+202C: terminates an embedding or override control
    pub const PopDirectionalFormat: BidiClass = BidiClass(16);
    /// (`NSM`) any nonspacing mark
    pub const NonspacingMark: BidiClass = BidiClass(17);
    /// (`BN`) most format characters, control codes, or noncharacters
    pub const BoundaryNeutral: BidiClass = BidiClass(18);
    /// (`FSI`) U+2068: the first strong isolate control
    pub const FirstStrongIsolate: BidiClass = BidiClass(19);
    /// (`LRI`) U+2066: the LR isolate control
    pub const LeftToRightIsolate: BidiClass = BidiClass(20);
    /// (`RLI`) U+2067: the RL isolate control
    pub const RightToLeftIsolate: BidiClass = BidiClass(21);
    /// (`PDI`) U+2069: terminates an isolate control
    pub const PopDirectionalIsolate: BidiClass = BidiClass(22);
}
}

make_enumerated_property! {
    name: "Bidi_Class";
    short_name: "bc";
    ident: BidiClass;
    data_marker: crate::provider::PropertyEnumBidiClassV1;
    singleton: SINGLETON_PROPERTY_ENUM_BIDI_CLASS_V1;
    ule_ty: u8;
}

// This exists to encapsulate GeneralCategoryULE so that it can exist in the provider module rather than props
pub(crate) mod gc {
    /// Enumerated property General_Category.
    ///
    /// General_Category specifies the most general classification of a code point, usually
    /// determined based on the primary characteristic of the assigned character. For example, is the
    /// character a letter, a mark, a number, punctuation, or a symbol, and if so, of what type?
    ///
    /// GeneralCategory only supports specific subcategories (eg `UppercaseLetter`).
    /// It does not support grouped categories (eg `Letter`). For grouped categories, use [`GeneralCategoryGroup`](
    /// crate::props::GeneralCategoryGroup).
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::{props::GeneralCategory, CodePointMapData};
    ///
    /// assert_eq!(
    ///     CodePointMapData::<GeneralCategory>::new().get('木'),
    ///     GeneralCategory::OtherLetter
    /// ); // U+6728
    /// assert_eq!(
    ///     CodePointMapData::<GeneralCategory>::new().get('🎃'),
    ///     GeneralCategory::OtherSymbol
    /// ); // U+1F383 JACK-O-LANTERN
    /// ```
    #[derive(Copy, Clone, PartialEq, Eq, Debug, Ord, PartialOrd, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "datagen", derive(databake::Bake))]
    #[cfg_attr(feature = "datagen", databake(path = icu_properties::props))]
    #[allow(clippy::exhaustive_enums)] // this type is stable
    #[zerovec::make_ule(GeneralCategoryULE)]
    #[cfg_attr(not(feature = "alloc"), zerovec::skip_derive(ZeroMapKV))]
    #[repr(u8)]
    pub enum GeneralCategory {
        /// (`Cn`) A reserved unassigned code point or a noncharacter
        Unassigned = 0,

        /// (`Lu`) An uppercase letter
        UppercaseLetter = 1,
        /// (`Ll`) A lowercase letter
        LowercaseLetter = 2,
        /// (`Lt`) A digraphic letter, with first part uppercase
        TitlecaseLetter = 3,
        /// (`Lm`) A modifier letter
        ModifierLetter = 4,
        /// (`Lo`) Other letters, including syllables and ideographs
        OtherLetter = 5,

        /// (`Mn`) A nonspacing combining mark (zero advance width)
        NonspacingMark = 6,
        /// (`Mc`) A spacing combining mark (positive advance width)
        SpacingMark = 8,
        /// (`Me`) An enclosing combining mark
        EnclosingMark = 7,

        /// (`Nd`) A decimal digit
        DecimalNumber = 9,
        /// (`Nl`) A letterlike numeric character
        LetterNumber = 10,
        /// (`No`) A numeric character of other type
        OtherNumber = 11,

        /// (`Zs`) A space character (of various non-zero widths)
        SpaceSeparator = 12,
        /// (`Zl`) U+2028 LINE SEPARATOR only
        LineSeparator = 13,
        /// (`Zp`) U+2029 PARAGRAPH SEPARATOR only
        ParagraphSeparator = 14,

        /// (`Cc`) A C0 or C1 control code
        Control = 15,
        /// (`Cf`) A format control character
        Format = 16,
        /// (`Co`) A private-use character
        PrivateUse = 17,
        /// (`Cs`) A surrogate code point
        Surrogate = 18,

        /// (`Pd`) A dash or hyphen punctuation mark
        DashPunctuation = 19,
        /// (`Ps`) An opening punctuation mark (of a pair)
        OpenPunctuation = 20,
        /// (`Pe`) A closing punctuation mark (of a pair)
        ClosePunctuation = 21,
        /// (`Pc`) A connecting punctuation mark, like a tie
        ConnectorPunctuation = 22,
        /// (`Pi`) An initial quotation mark
        InitialPunctuation = 28,
        /// (`Pf`) A final quotation mark
        FinalPunctuation = 29,
        /// (`Po`) A punctuation mark of other type
        OtherPunctuation = 23,

        /// (`Sm`) A symbol of mathematical use
        MathSymbol = 24,
        /// (`Sc`) A currency sign
        CurrencySymbol = 25,
        /// (`Sk`) A non-letterlike modifier symbol
        ModifierSymbol = 26,
        /// (`So`) A symbol of other type
        OtherSymbol = 27,
    }
}

pub use gc::GeneralCategory;

impl GeneralCategory {
    /// All possible values of this enum
    pub const ALL_VALUES: &'static [GeneralCategory] = &[
        GeneralCategory::Unassigned,
        GeneralCategory::UppercaseLetter,
        GeneralCategory::LowercaseLetter,
        GeneralCategory::TitlecaseLetter,
        GeneralCategory::ModifierLetter,
        GeneralCategory::OtherLetter,
        GeneralCategory::NonspacingMark,
        GeneralCategory::SpacingMark,
        GeneralCategory::EnclosingMark,
        GeneralCategory::DecimalNumber,
        GeneralCategory::LetterNumber,
        GeneralCategory::OtherNumber,
        GeneralCategory::SpaceSeparator,
        GeneralCategory::LineSeparator,
        GeneralCategory::ParagraphSeparator,
        GeneralCategory::Control,
        GeneralCategory::Format,
        GeneralCategory::PrivateUse,
        GeneralCategory::Surrogate,
        GeneralCategory::DashPunctuation,
        GeneralCategory::OpenPunctuation,
        GeneralCategory::ClosePunctuation,
        GeneralCategory::ConnectorPunctuation,
        GeneralCategory::InitialPunctuation,
        GeneralCategory::FinalPunctuation,
        GeneralCategory::OtherPunctuation,
        GeneralCategory::MathSymbol,
        GeneralCategory::CurrencySymbol,
        GeneralCategory::ModifierSymbol,
        GeneralCategory::OtherSymbol,
    ];
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default)]
/// Error value for `impl TryFrom<u8> for GeneralCategory`.
#[non_exhaustive]
pub struct GeneralCategoryOutOfBoundsError;

impl TryFrom<u8> for GeneralCategory {
    type Error = GeneralCategoryOutOfBoundsError;
    /// Construct this [`GeneralCategory`] from an integer, returning
    /// an error if it is out of bounds
    fn try_from(val: u8) -> Result<Self, GeneralCategoryOutOfBoundsError> {
        GeneralCategory::new_from_u8(val).ok_or(GeneralCategoryOutOfBoundsError)
    }
}

make_enumerated_property! {
    name: "General_Category";
    short_name: "gc";
    ident: GeneralCategory;
    data_marker: crate::provider::PropertyEnumGeneralCategoryV1;
    singleton: SINGLETON_PROPERTY_ENUM_GENERAL_CATEGORY_V1;
}

/// Groupings of multiple General_Category property values.
///
/// Instances of `GeneralCategoryGroup` represent the defined multi-category
/// values that are useful for users in certain contexts, such as regex. In
/// other words, unlike [`GeneralCategory`], this supports groups of general
/// categories: for example, `Letter` /// is the union of `UppercaseLetter`,
/// `LowercaseLetter`, etc.
///
/// See <https://www.unicode.org/reports/tr44/> .
///
/// The discriminants correspond to the `U_GC_XX_MASK` constants in ICU4C.
/// Unlike [`GeneralCategory`], this supports groups of general categories: for example, `Letter`
/// is the union of `UppercaseLetter`, `LowercaseLetter`, etc.
///
/// See `UCharCategory` and `U_GET_GC_MASK` in ICU4C.
#[derive(Copy, Clone, PartialEq, Debug, Eq)]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct GeneralCategoryGroup(pub(crate) u32);

impl crate::private::Sealed for GeneralCategoryGroup {}

use GeneralCategory as GC;
use GeneralCategoryGroup as GCG;

#[allow(non_upper_case_globals)]
impl GeneralCategoryGroup {
    /// (`Lu`) An uppercase letter
    pub const UppercaseLetter: GeneralCategoryGroup = GCG(1 << (GC::UppercaseLetter as u32));
    /// (`Ll`) A lowercase letter
    pub const LowercaseLetter: GeneralCategoryGroup = GCG(1 << (GC::LowercaseLetter as u32));
    /// (`Lt`) A digraphic letter, with first part uppercase
    pub const TitlecaseLetter: GeneralCategoryGroup = GCG(1 << (GC::TitlecaseLetter as u32));
    /// (`Lm`) A modifier letter
    pub const ModifierLetter: GeneralCategoryGroup = GCG(1 << (GC::ModifierLetter as u32));
    /// (`Lo`) Other letters, including syllables and ideographs
    pub const OtherLetter: GeneralCategoryGroup = GCG(1 << (GC::OtherLetter as u32));
    /// (`LC`) The union of UppercaseLetter, LowercaseLetter, and TitlecaseLetter
    pub const CasedLetter: GeneralCategoryGroup = GCG((1 << (GC::UppercaseLetter as u32))
        | (1 << (GC::LowercaseLetter as u32))
        | (1 << (GC::TitlecaseLetter as u32)));
    /// (`L`) The union of all letter categories
    pub const Letter: GeneralCategoryGroup = GCG((1 << (GC::UppercaseLetter as u32))
        | (1 << (GC::LowercaseLetter as u32))
        | (1 << (GC::TitlecaseLetter as u32))
        | (1 << (GC::ModifierLetter as u32))
        | (1 << (GC::OtherLetter as u32)));

    /// (`Mn`) A nonspacing combining mark (zero advance width)
    pub const NonspacingMark: GeneralCategoryGroup = GCG(1 << (GC::NonspacingMark as u32));
    /// (`Mc`) A spacing combining mark (positive advance width)
    pub const EnclosingMark: GeneralCategoryGroup = GCG(1 << (GC::EnclosingMark as u32));
    /// (`Me`) An enclosing combining mark
    pub const SpacingMark: GeneralCategoryGroup = GCG(1 << (GC::SpacingMark as u32));
    /// (`M`) The union of all mark categories
    pub const Mark: GeneralCategoryGroup = GCG((1 << (GC::NonspacingMark as u32))
        | (1 << (GC::EnclosingMark as u32))
        | (1 << (GC::SpacingMark as u32)));

    /// (`Nd`) A decimal digit
    pub const DecimalNumber: GeneralCategoryGroup = GCG(1 << (GC::DecimalNumber as u32));
    /// (`Nl`) A letterlike numeric character
    pub const LetterNumber: GeneralCategoryGroup = GCG(1 << (GC::LetterNumber as u32));
    /// (`No`) A numeric character of other type
    pub const OtherNumber: GeneralCategoryGroup = GCG(1 << (GC::OtherNumber as u32));
    /// (`N`) The union of all number categories
    pub const Number: GeneralCategoryGroup = GCG((1 << (GC::DecimalNumber as u32))
        | (1 << (GC::LetterNumber as u32))
        | (1 << (GC::OtherNumber as u32)));

    /// (`Zs`) A space character (of various non-zero widths)
    pub const SpaceSeparator: GeneralCategoryGroup = GCG(1 << (GC::SpaceSeparator as u32));
    /// (`Zl`) U+2028 LINE SEPARATOR only
    pub const LineSeparator: GeneralCategoryGroup = GCG(1 << (GC::LineSeparator as u32));
    /// (`Zp`) U+2029 PARAGRAPH SEPARATOR only
    pub const ParagraphSeparator: GeneralCategoryGroup = GCG(1 << (GC::ParagraphSeparator as u32));
    /// (`Z`) The union of all separator categories
    pub const Separator: GeneralCategoryGroup = GCG((1 << (GC::SpaceSeparator as u32))
        | (1 << (GC::LineSeparator as u32))
        | (1 << (GC::ParagraphSeparator as u32)));

    /// (`Cc`) A C0 or C1 control code
    pub const Control: GeneralCategoryGroup = GCG(1 << (GC::Control as u32));
    /// (`Cf`) A format control character
    pub const Format: GeneralCategoryGroup = GCG(1 << (GC::Format as u32));
    /// (`Co`) A private-use character
    pub const PrivateUse: GeneralCategoryGroup = GCG(1 << (GC::PrivateUse as u32));
    /// (`Cs`) A surrogate code point
    pub const Surrogate: GeneralCategoryGroup = GCG(1 << (GC::Surrogate as u32));
    /// (`Cn`) A reserved unassigned code point or a noncharacter
    pub const Unassigned: GeneralCategoryGroup = GCG(1 << (GC::Unassigned as u32));
    /// (`C`) The union of all control code, reserved, and unassigned categories
    pub const Other: GeneralCategoryGroup = GCG((1 << (GC::Control as u32))
        | (1 << (GC::Format as u32))
        | (1 << (GC::PrivateUse as u32))
        | (1 << (GC::Surrogate as u32))
        | (1 << (GC::Unassigned as u32)));

    /// (`Pd`) A dash or hyphen punctuation mark
    pub const DashPunctuation: GeneralCategoryGroup = GCG(1 << (GC::DashPunctuation as u32));
    /// (`Ps`) An opening punctuation mark (of a pair)
    pub const OpenPunctuation: GeneralCategoryGroup = GCG(1 << (GC::OpenPunctuation as u32));
    /// (`Pe`) A closing punctuation mark (of a pair)
    pub const ClosePunctuation: GeneralCategoryGroup = GCG(1 << (GC::ClosePunctuation as u32));
    /// (`Pc`) A connecting punctuation mark, like a tie
    pub const ConnectorPunctuation: GeneralCategoryGroup =
        GCG(1 << (GC::ConnectorPunctuation as u32));
    /// (`Pi`) An initial quotation mark
    pub const InitialPunctuation: GeneralCategoryGroup = GCG(1 << (GC::InitialPunctuation as u32));
    /// (`Pf`) A final quotation mark
    pub const FinalPunctuation: GeneralCategoryGroup = GCG(1 << (GC::FinalPunctuation as u32));
    /// (`Po`) A punctuation mark of other type
    pub const OtherPunctuation: GeneralCategoryGroup = GCG(1 << (GC::OtherPunctuation as u32));
    /// (`P`) The union of all punctuation categories
    pub const Punctuation: GeneralCategoryGroup = GCG((1 << (GC::DashPunctuation as u32))
        | (1 << (GC::OpenPunctuation as u32))
        | (1 << (GC::ClosePunctuation as u32))
        | (1 << (GC::ConnectorPunctuation as u32))
        | (1 << (GC::OtherPunctuation as u32))
        | (1 << (GC::InitialPunctuation as u32))
        | (1 << (GC::FinalPunctuation as u32)));

    /// (`Sm`) A symbol of mathematical use
    pub const MathSymbol: GeneralCategoryGroup = GCG(1 << (GC::MathSymbol as u32));
    /// (`Sc`) A currency sign
    pub const CurrencySymbol: GeneralCategoryGroup = GCG(1 << (GC::CurrencySymbol as u32));
    /// (`Sk`) A non-letterlike modifier symbol
    pub const ModifierSymbol: GeneralCategoryGroup = GCG(1 << (GC::ModifierSymbol as u32));
    /// (`So`) A symbol of other type
    pub const OtherSymbol: GeneralCategoryGroup = GCG(1 << (GC::OtherSymbol as u32));
    /// (`S`) The union of all symbol categories
    pub const Symbol: GeneralCategoryGroup = GCG((1 << (GC::MathSymbol as u32))
        | (1 << (GC::CurrencySymbol as u32))
        | (1 << (GC::ModifierSymbol as u32))
        | (1 << (GC::OtherSymbol as u32)));

    const ALL: u32 = (1 << (GC::FinalPunctuation as u32 + 1)) - 1;

    /// Return whether the code point belongs in the provided multi-value category.
    ///
    /// ```
    /// use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};
    /// use icu::properties::CodePointMapData;
    ///
    /// let gc = CodePointMapData::<GeneralCategory>::new();
    ///
    /// assert_eq!(gc.get('A'), GeneralCategory::UppercaseLetter);
    /// assert!(GeneralCategoryGroup::CasedLetter.contains(gc.get('A')));
    ///
    /// // U+0B1E ORIYA LETTER NYA
    /// assert_eq!(gc.get('ଞ'), GeneralCategory::OtherLetter);
    /// assert!(GeneralCategoryGroup::Letter.contains(gc.get('ଞ')));
    /// assert!(!GeneralCategoryGroup::CasedLetter.contains(gc.get('ଞ')));
    ///
    /// // U+0301 COMBINING ACUTE ACCENT
    /// assert_eq!(gc.get('\u{0301}'), GeneralCategory::NonspacingMark);
    /// assert!(GeneralCategoryGroup::Mark.contains(gc.get('\u{0301}')));
    /// assert!(!GeneralCategoryGroup::Letter.contains(gc.get('\u{0301}')));
    ///
    /// assert_eq!(gc.get('0'), GeneralCategory::DecimalNumber);
    /// assert!(GeneralCategoryGroup::Number.contains(gc.get('0')));
    /// assert!(!GeneralCategoryGroup::Mark.contains(gc.get('0')));
    ///
    /// assert_eq!(gc.get('('), GeneralCategory::OpenPunctuation);
    /// assert!(GeneralCategoryGroup::Punctuation.contains(gc.get('(')));
    /// assert!(!GeneralCategoryGroup::Number.contains(gc.get('(')));
    ///
    /// // U+2713 CHECK MARK
    /// assert_eq!(gc.get('✓'), GeneralCategory::OtherSymbol);
    /// assert!(GeneralCategoryGroup::Symbol.contains(gc.get('✓')));
    /// assert!(!GeneralCategoryGroup::Punctuation.contains(gc.get('✓')));
    ///
    /// assert_eq!(gc.get(' '), GeneralCategory::SpaceSeparator);
    /// assert!(GeneralCategoryGroup::Separator.contains(gc.get(' ')));
    /// assert!(!GeneralCategoryGroup::Symbol.contains(gc.get(' ')));
    ///
    /// // U+E007F CANCEL TAG
    /// assert_eq!(gc.get('\u{E007F}'), GeneralCategory::Format);
    /// assert!(GeneralCategoryGroup::Other.contains(gc.get('\u{E007F}')));
    /// assert!(!GeneralCategoryGroup::Separator.contains(gc.get('\u{E007F}')));
    /// ```
    pub const fn contains(self, val: GeneralCategory) -> bool {
        0 != (1 << (val as u32)) & self.0
    }

    /// Produce a GeneralCategoryGroup that is the inverse of this one
    ///
    /// # Example
    ///
    /// ```rust
    /// use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};
    ///
    /// let letter = GeneralCategoryGroup::Letter;
    /// let not_letter = letter.complement();
    ///
    /// assert!(not_letter.contains(GeneralCategory::MathSymbol));
    /// assert!(!letter.contains(GeneralCategory::MathSymbol));
    /// assert!(not_letter.contains(GeneralCategory::OtherPunctuation));
    /// assert!(!letter.contains(GeneralCategory::OtherPunctuation));
    /// assert!(!not_letter.contains(GeneralCategory::UppercaseLetter));
    /// assert!(letter.contains(GeneralCategory::UppercaseLetter));
    /// ```
    pub const fn complement(self) -> Self {
        // Mask off things not in Self::ALL to guarantee the mask
        // values stay in-range
        GeneralCategoryGroup(!self.0 & Self::ALL)
    }

    /// Return the group representing all GeneralCategory values
    ///
    /// # Example
    ///
    /// ```rust
    /// use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};
    ///
    /// let all = GeneralCategoryGroup::all();
    ///
    /// assert!(all.contains(GeneralCategory::MathSymbol));
    /// assert!(all.contains(GeneralCategory::OtherPunctuation));
    /// assert!(all.contains(GeneralCategory::UppercaseLetter));
    /// ```
    pub const fn all() -> Self {
        Self(Self::ALL)
    }

    /// Return the empty group
    ///
    /// # Example
    ///
    /// ```rust
    /// use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};
    ///
    /// let empty = GeneralCategoryGroup::empty();
    ///
    /// assert!(!empty.contains(GeneralCategory::MathSymbol));
    /// assert!(!empty.contains(GeneralCategory::OtherPunctuation));
    /// assert!(!empty.contains(GeneralCategory::UppercaseLetter));
    /// ```
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Take the union of two groups
    ///
    /// # Example
    ///
    /// ```rust
    /// use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};
    ///
    /// let letter = GeneralCategoryGroup::Letter;
    /// let symbol = GeneralCategoryGroup::Symbol;
    /// let union = letter.union(symbol);
    ///
    /// assert!(union.contains(GeneralCategory::MathSymbol));
    /// assert!(!union.contains(GeneralCategory::OtherPunctuation));
    /// assert!(union.contains(GeneralCategory::UppercaseLetter));
    /// ```
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Take the intersection of two groups
    ///
    /// # Example
    ///
    /// ```rust
    /// use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};
    ///
    /// let letter = GeneralCategoryGroup::Letter;
    /// let lu = GeneralCategoryGroup::UppercaseLetter;
    /// let intersection = letter.intersection(lu);
    ///
    /// assert!(!intersection.contains(GeneralCategory::MathSymbol));
    /// assert!(!intersection.contains(GeneralCategory::OtherPunctuation));
    /// assert!(intersection.contains(GeneralCategory::UppercaseLetter));
    /// assert!(!intersection.contains(GeneralCategory::LowercaseLetter));
    /// ```
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

impl From<GeneralCategory> for GeneralCategoryGroup {
    fn from(subcategory: GeneralCategory) -> Self {
        GeneralCategoryGroup(1 << (subcategory as u32))
    }
}
impl From<u32> for GeneralCategoryGroup {
    fn from(mask: u32) -> Self {
        // Mask off things not in Self::ALL to guarantee the mask
        // values stay in-range
        GeneralCategoryGroup(mask & Self::ALL)
    }
}
impl From<GeneralCategoryGroup> for u32 {
    fn from(group: GeneralCategoryGroup) -> Self {
        group.0
    }
}

/// Enumerated property Script.
///
/// This is used with both the Script and Script_Extensions Unicode properties.
/// Each character is assigned a single Script, but characters that are used in
/// a particular subset of scripts will be in more than one Script_Extensions set.
/// For example, DEVANAGARI DIGIT NINE has Script=Devanagari, but is also in the
/// Script_Extensions set for Dogra, Kaithi, and Mahajani. If you are trying to
/// determine whether a code point belongs to a certain script, you should use
/// [`ScriptWithExtensionsBorrowed::has_script`].
///
/// For more information, see UAX #24: <http://www.unicode.org/reports/tr24/>.
/// See `UScriptCode` in ICU4C.
///
/// # Example
///
/// ```
/// use icu::properties::{CodePointMapData, props::Script};
///
/// assert_eq!(CodePointMapData::<Script>::new().get('木'), Script::Han);  // U+6728
/// assert_eq!(CodePointMapData::<Script>::new().get('🎃'), Script::Common);  // U+1F383 JACK-O-LANTERN
/// ```
/// [`ScriptWithExtensionsBorrowed::has_script`]: crate::script::ScriptWithExtensionsBorrowed::has_script
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct Script(pub(crate) u16);

impl Script {
    /// Returns an ICU4C `UScriptCode` value.
    pub const fn to_icu4c_value(self) -> u16 {
        self.0
    }
    /// Constructor from an ICU4C `UScriptCode` value.
    pub const fn from_icu4c_value(value: u16) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
impl Script {
    pub const Adlam: Script = Script(167);
    pub const Ahom: Script = Script(161);
    pub const AnatolianHieroglyphs: Script = Script(156);
    pub const Arabic: Script = Script(2);
    pub const Armenian: Script = Script(3);
    pub const Avestan: Script = Script(117);
    pub const Balinese: Script = Script(62);
    pub const Bamum: Script = Script(130);
    pub const BassaVah: Script = Script(134);
    pub const Batak: Script = Script(63);
    pub const Bengali: Script = Script(4);
    pub const BeriaErfe: Script = Script(208);
    pub const Bhaiksuki: Script = Script(168);
    pub const Bopomofo: Script = Script(5);
    pub const Brahmi: Script = Script(65);
    pub const Braille: Script = Script(46);
    pub const Buginese: Script = Script(55);
    pub const Buhid: Script = Script(44);
    pub const CanadianAboriginal: Script = Script(40);
    pub const Carian: Script = Script(104);
    pub const CaucasianAlbanian: Script = Script(159);
    pub const Chakma: Script = Script(118);
    pub const Cham: Script = Script(66);
    pub const Cherokee: Script = Script(6);
    pub const Chisoi: Script = Script(209);
    pub const Chorasmian: Script = Script(189);
    pub const Common: Script = Script(0);
    pub const Coptic: Script = Script(7);
    pub const Cuneiform: Script = Script(101);
    pub const Cypriot: Script = Script(47);
    pub const CyproMinoan: Script = Script(193);
    pub const Cyrillic: Script = Script(8);
    pub const Deseret: Script = Script(9);
    pub const Devanagari: Script = Script(10);
    pub const DivesAkuru: Script = Script(190);
    pub const Dogra: Script = Script(178);
    pub const Duployan: Script = Script(135);
    pub const EgyptianHieroglyphs: Script = Script(71);
    pub const Elbasan: Script = Script(136);
    pub const Elymaic: Script = Script(185);
    pub const Ethiopian: Script = Script(11);
    pub const Georgian: Script = Script(12);
    pub const Glagolitic: Script = Script(56);
    pub const Gothic: Script = Script(13);
    pub const Grantha: Script = Script(137);
    pub const Greek: Script = Script(14);
    pub const Gujarati: Script = Script(15);
    pub const GunjalaGondi: Script = Script(179);
    pub const Gurmukhi: Script = Script(16);
    pub const Han: Script = Script(17);
    pub const Hangul: Script = Script(18);
    pub const HanifiRohingya: Script = Script(182);
    pub const Hanunoo: Script = Script(43);
    pub const Hatran: Script = Script(162);
    pub const Hebrew: Script = Script(19);
    pub const Hiragana: Script = Script(20);
    pub const ImperialAramaic: Script = Script(116);
    pub const Inherited: Script = Script(1);
    pub const InscriptionalPahlavi: Script = Script(122);
    pub const InscriptionalParthian: Script = Script(125);
    pub const Javanese: Script = Script(78);
    pub const Kaithi: Script = Script(120);
    pub const Kannada: Script = Script(21);
    pub const Katakana: Script = Script(22);
    pub const Kawi: Script = Script(198);
    pub const KayahLi: Script = Script(79);
    pub const Kharoshthi: Script = Script(57);
    pub const KhitanSmallScript: Script = Script(191);
    pub const Khmer: Script = Script(23);
    pub const Khojki: Script = Script(157);
    pub const Khudawadi: Script = Script(145);
    pub const Lao: Script = Script(24);
    pub const Latin: Script = Script(25);
    pub const Lepcha: Script = Script(82);
    pub const Limbu: Script = Script(48);
    pub const LinearA: Script = Script(83);
    pub const LinearB: Script = Script(49);
    pub const Lisu: Script = Script(131);
    pub const Lycian: Script = Script(107);
    pub const Lydian: Script = Script(108);
    pub const Mahajani: Script = Script(160);
    pub const Makasar: Script = Script(180);
    pub const Malayalam: Script = Script(26);
    pub const Mandaic: Script = Script(84);
    pub const Manichaean: Script = Script(121);
    pub const Marchen: Script = Script(169);
    pub const MasaramGondi: Script = Script(175);
    pub const Medefaidrin: Script = Script(181);
    pub const MeeteiMayek: Script = Script(115);
    pub const MendeKikakui: Script = Script(140);
    pub const MeroiticCursive: Script = Script(141);
    pub const MeroiticHieroglyphs: Script = Script(86);
    pub const Miao: Script = Script(92);
    pub const Modi: Script = Script(163);
    pub const Mongolian: Script = Script(27);
    pub const Mro: Script = Script(149);
    pub const Multani: Script = Script(164);
    pub const Myanmar: Script = Script(28);
    pub const Nabataean: Script = Script(143);
    pub const NagMundari: Script = Script(199);
    pub const Nandinagari: Script = Script(187);
    pub const Nastaliq: Script = Script(200);
    pub const Newa: Script = Script(170);
    pub const NewTaiLue: Script = Script(59);
    pub const Nko: Script = Script(87);
    pub const Nushu: Script = Script(150);
    pub const NyiakengPuachueHmong: Script = Script(186);
    pub const Ogham: Script = Script(29);
    pub const OlChiki: Script = Script(109);
    pub const OldHungarian: Script = Script(76);
    pub const OldItalic: Script = Script(30);
    pub const OldNorthArabian: Script = Script(142);
    pub const OldPermic: Script = Script(89);
    pub const OldPersian: Script = Script(61);
    pub const OldSogdian: Script = Script(184);
    pub const OldSouthArabian: Script = Script(133);
    pub const OldTurkic: Script = Script(88);
    pub const OldUyghur: Script = Script(194);
    pub const Oriya: Script = Script(31);
    pub const Osage: Script = Script(171);
    pub const Osmanya: Script = Script(50);
    pub const PahawhHmong: Script = Script(75);
    pub const Palmyrene: Script = Script(144);
    pub const PauCinHau: Script = Script(165);
    pub const PhagsPa: Script = Script(90);
    pub const Phoenician: Script = Script(91);
    pub const PsalterPahlavi: Script = Script(123);
    pub const Rejang: Script = Script(110);
    pub const Runic: Script = Script(32);
    pub const Samaritan: Script = Script(126);
    pub const Saurashtra: Script = Script(111);
    pub const Sharada: Script = Script(151);
    pub const Shavian: Script = Script(51);
    pub const Siddham: Script = Script(166);
    pub const Sidetic: Script = Script(210);
    pub const SignWriting: Script = Script(112);
    pub const Sinhala: Script = Script(33);
    pub const Sogdian: Script = Script(183);
    pub const SoraSompeng: Script = Script(152);
    pub const Soyombo: Script = Script(176);
    pub const Sundanese: Script = Script(113);
    pub const SylotiNagri: Script = Script(58);
    pub const Syriac: Script = Script(34);
    pub const Tagalog: Script = Script(42);
    pub const Tagbanwa: Script = Script(45);
    pub const TaiLe: Script = Script(52);
    pub const TaiTham: Script = Script(106);
    pub const TaiViet: Script = Script(127);
    pub const TaiYo: Script = Script(211);
    pub const Takri: Script = Script(153);
    pub const Tamil: Script = Script(35);
    pub const Tangsa: Script = Script(195);
    pub const Tangut: Script = Script(154);
    pub const Telugu: Script = Script(36);
    pub const Thaana: Script = Script(37);
    pub const Thai: Script = Script(38);
    pub const Tibetan: Script = Script(39);
    pub const Tifinagh: Script = Script(60);
    pub const Tirhuta: Script = Script(158);
    pub const TolongSiki: Script = Script(212);
    pub const Toto: Script = Script(196);
    pub const Ugaritic: Script = Script(53);
    pub const Unknown: Script = Script(103);
    pub const Vai: Script = Script(99);
    pub const Vithkuqi: Script = Script(197);
    pub const Wancho: Script = Script(188);
    pub const WarangCiti: Script = Script(146);
    pub const Yezidi: Script = Script(192);
    pub const Yi: Script = Script(41);
    pub const ZanabazarSquare: Script = Script(177);
}
}

make_enumerated_property! {
    name: "Script";
    short_name: "sc";
    ident: Script;
    data_marker: crate::provider::PropertyEnumScriptV1;
    singleton: SINGLETON_PROPERTY_ENUM_SCRIPT_V1;
    ule_ty: <u16 as zerovec::ule::AsULE>::ULE;
}

/// Enumerated property Hangul_Syllable_Type
///
/// The Unicode standard provides both precomposed Hangul syllables and conjoining Jamo to compose
/// arbitrary Hangul syllables. This property provides that ontology of Hangul code points.
///
/// For more information, see the [Unicode Korean FAQ](https://www.unicode.org/faq/korean.html).
///
/// # Example
///
/// ```
/// use icu::properties::{props::HangulSyllableType, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<HangulSyllableType>::new().get('ᄀ'),
///     HangulSyllableType::LeadingJamo
/// ); // U+1100
/// assert_eq!(
///     CodePointMapData::<HangulSyllableType>::new().get('가'),
///     HangulSyllableType::LeadingVowelSyllable
/// ); // U+AC00
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct HangulSyllableType(pub(crate) u8);

impl HangulSyllableType {
    /// Returns an ICU4C `UHangulSyllableType` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `UHangulSyllableType` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(non_upper_case_globals)]
impl HangulSyllableType {
    /// (`NA`) not applicable (e.g. not a Hangul code point).
    pub const NotApplicable: HangulSyllableType = HangulSyllableType(0);
    /// (`L`) a conjoining leading consonant Jamo.
    pub const LeadingJamo: HangulSyllableType = HangulSyllableType(1);
    /// (`V`) a conjoining vowel Jamo.
    pub const VowelJamo: HangulSyllableType = HangulSyllableType(2);
    /// (`T`) a conjoining trailing consonant Jamo.
    pub const TrailingJamo: HangulSyllableType = HangulSyllableType(3);
    /// (`LV`) a precomposed syllable with a leading consonant and a vowel.
    pub const LeadingVowelSyllable: HangulSyllableType = HangulSyllableType(4);
    /// (`LVT`) a precomposed syllable with a leading consonant, a vowel, and a trailing consonant.
    pub const LeadingVowelTrailingSyllable: HangulSyllableType = HangulSyllableType(5);
}
}

make_enumerated_property! {
    name: "Hangul_Syllable_Type";
    short_name: "hst";
    ident: HangulSyllableType;
    data_marker: crate::provider::PropertyEnumHangulSyllableTypeV1;
    singleton: SINGLETON_PROPERTY_ENUM_HANGUL_SYLLABLE_TYPE_V1;
    ule_ty: u8;

}

/// Enumerated property East_Asian_Width.
///
/// See "Definition" in UAX #11 for the summary of each property value:
/// <https://www.unicode.org/reports/tr11/#Definitions>
///
/// # Example
///
/// ```
/// use icu::properties::{props::EastAsianWidth, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<EastAsianWidth>::new().get('ｱ'),
///     EastAsianWidth::Halfwidth
/// ); // U+FF71: Halfwidth Katakana Letter A
/// assert_eq!(
///     CodePointMapData::<EastAsianWidth>::new().get('ア'),
///     EastAsianWidth::Wide
/// ); //U+30A2: Katakana Letter A
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct EastAsianWidth(pub(crate) u8);

impl EastAsianWidth {
    /// Returns an ICU4C `UEastAsianWidth` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `UEastAsianWidth` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
impl EastAsianWidth {
    pub const Neutral: EastAsianWidth = EastAsianWidth(0); //name="N"
    pub const Ambiguous: EastAsianWidth = EastAsianWidth(1); //name="A"
    pub const Halfwidth: EastAsianWidth = EastAsianWidth(2); //name="H"
    pub const Fullwidth: EastAsianWidth = EastAsianWidth(3); //name="F"
    pub const Narrow: EastAsianWidth = EastAsianWidth(4); //name="Na"
    pub const Wide: EastAsianWidth = EastAsianWidth(5); //name="W"
}
}

make_enumerated_property! {
    name: "East_Asian_Width";
    short_name: "ea";
    ident: EastAsianWidth;
    data_marker: crate::provider::PropertyEnumEastAsianWidthV1;
    singleton: SINGLETON_PROPERTY_ENUM_EAST_ASIAN_WIDTH_V1;
    ule_ty: u8;
}

/// Enumerated property Line_Break.
///
/// See "Line Breaking Properties" in UAX #14 for the summary of each property
/// value: <https://www.unicode.org/reports/tr14/#Properties>
///
/// The numeric value is compatible with `ULineBreak` in ICU4C.
///
/// **Note:** Use `icu::segmenter` for an all-in-one break iterator implementation.
///
/// # Example
///
/// ```
/// use icu::properties::{props::LineBreak, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<LineBreak>::new().get(')'),
///     LineBreak::CloseParenthesis
/// ); // U+0029: Right Parenthesis
/// assert_eq!(
///     CodePointMapData::<LineBreak>::new().get('ぁ'),
///     LineBreak::ConditionalJapaneseStarter
/// ); //U+3041: Hiragana Letter Small A
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct LineBreak(pub(crate) u8);

impl LineBreak {
    /// Returns an ICU4C `ULineBreak` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `ULineBreak` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
impl LineBreak {
    pub const Unknown: LineBreak = LineBreak(0); // name="XX"
    pub const Ambiguous: LineBreak = LineBreak(1); // name="AI"
    pub const Alphabetic: LineBreak = LineBreak(2); // name="AL"
    pub const BreakBoth: LineBreak = LineBreak(3); // name="B2"
    pub const BreakAfter: LineBreak = LineBreak(4); // name="BA"
    pub const BreakBefore: LineBreak = LineBreak(5); // name="BB"
    pub const MandatoryBreak: LineBreak = LineBreak(6); // name="BK"
    pub const ContingentBreak: LineBreak = LineBreak(7); // name="CB"
    pub const ClosePunctuation: LineBreak = LineBreak(8); // name="CL"
    pub const CombiningMark: LineBreak = LineBreak(9); // name="CM"
    pub const CarriageReturn: LineBreak = LineBreak(10); // name="CR"
    pub const Exclamation: LineBreak = LineBreak(11); // name="EX"
    pub const Glue: LineBreak = LineBreak(12); // name="GL"
    pub const Hyphen: LineBreak = LineBreak(13); // name="HY"
    pub const Ideographic: LineBreak = LineBreak(14); // name="ID"
    pub const Inseparable: LineBreak = LineBreak(15); // name="IN"
    pub const InfixNumeric: LineBreak = LineBreak(16); // name="IS"
    pub const LineFeed: LineBreak = LineBreak(17); // name="LF"
    pub const Nonstarter: LineBreak = LineBreak(18); // name="NS"
    pub const Numeric: LineBreak = LineBreak(19); // name="NU"
    pub const OpenPunctuation: LineBreak = LineBreak(20); // name="OP"
    pub const PostfixNumeric: LineBreak = LineBreak(21); // name="PO"
    pub const PrefixNumeric: LineBreak = LineBreak(22); // name="PR"
    pub const Quotation: LineBreak = LineBreak(23); // name="QU"
    pub const ComplexContext: LineBreak = LineBreak(24); // name="SA"
    pub const Surrogate: LineBreak = LineBreak(25); // name="SG"
    pub const Space: LineBreak = LineBreak(26); // name="SP"
    pub const BreakSymbols: LineBreak = LineBreak(27); // name="SY"
    pub const ZWSpace: LineBreak = LineBreak(28); // name="ZW"
    pub const NextLine: LineBreak = LineBreak(29); // name="NL"
    pub const WordJoiner: LineBreak = LineBreak(30); // name="WJ"
    pub const H2: LineBreak = LineBreak(31); // name="H2"
    pub const H3: LineBreak = LineBreak(32); // name="H3"
    pub const JL: LineBreak = LineBreak(33); // name="JL"
    pub const JT: LineBreak = LineBreak(34); // name="JT"
    pub const JV: LineBreak = LineBreak(35); // name="JV"
    pub const CloseParenthesis: LineBreak = LineBreak(36); // name="CP"
    pub const ConditionalJapaneseStarter: LineBreak = LineBreak(37); // name="CJ"
    pub const HebrewLetter: LineBreak = LineBreak(38); // name="HL"
    pub const RegionalIndicator: LineBreak = LineBreak(39); // name="RI"
    pub const EBase: LineBreak = LineBreak(40); // name="EB"
    pub const EModifier: LineBreak = LineBreak(41); // name="EM"
    pub const ZWJ: LineBreak = LineBreak(42); // name="ZWJ"

    // Added in ICU 74:
    pub const Aksara: LineBreak = LineBreak(43); // name="AK"
    pub const AksaraPrebase: LineBreak = LineBreak(44); // name="AP"
    pub const AksaraStart: LineBreak = LineBreak(45); // name="AS"
    pub const ViramaFinal: LineBreak = LineBreak(46); // name="VF"
    pub const Virama: LineBreak = LineBreak(47); // name="VI"

    // Added in ICU 78:
    pub const UnambiguousHyphen: LineBreak = LineBreak(48); // name="HH"
}
}

make_enumerated_property! {
    name: "Line_Break";
    short_name: "lb";
    ident: LineBreak;
    data_marker: crate::provider::PropertyEnumLineBreakV1;
    singleton: SINGLETON_PROPERTY_ENUM_LINE_BREAK_V1;
    ule_ty: u8;
}

/// Enumerated property Grapheme_Cluster_Break.
///
/// See "Default Grapheme Cluster Boundary Specification" in UAX #29 for the
/// summary of each property value:
/// <https://www.unicode.org/reports/tr29/#Default_Grapheme_Cluster_Table>
///
/// **Note:** Use `icu::segmenter` for an all-in-one break iterator implementation.
///
/// # Example
///
/// ```
/// use icu::properties::{props::GraphemeClusterBreak, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<GraphemeClusterBreak>::new().get('🇦'),
///     GraphemeClusterBreak::RegionalIndicator
/// ); // U+1F1E6: Regional Indicator Symbol Letter A
/// assert_eq!(
///     CodePointMapData::<GraphemeClusterBreak>::new().get('ำ'),
///     GraphemeClusterBreak::SpacingMark
/// ); //U+0E33: Thai Character Sara Am
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // this type is stable
#[repr(transparent)]
pub struct GraphemeClusterBreak(pub(crate) u8);

impl GraphemeClusterBreak {
    /// Returns an ICU4C `UGraphemeClusterBreak` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `UGraphemeClusterBreak` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
impl GraphemeClusterBreak {
    pub const Other: GraphemeClusterBreak = GraphemeClusterBreak(0); // name="XX"
    pub const Control: GraphemeClusterBreak = GraphemeClusterBreak(1); // name="CN"
    pub const CR: GraphemeClusterBreak = GraphemeClusterBreak(2); // name="CR"
    pub const Extend: GraphemeClusterBreak = GraphemeClusterBreak(3); // name="EX"
    pub const L: GraphemeClusterBreak = GraphemeClusterBreak(4); // name="L"
    pub const LF: GraphemeClusterBreak = GraphemeClusterBreak(5); // name="LF"
    pub const LV: GraphemeClusterBreak = GraphemeClusterBreak(6); // name="LV"
    pub const LVT: GraphemeClusterBreak = GraphemeClusterBreak(7); // name="LVT"
    pub const T: GraphemeClusterBreak = GraphemeClusterBreak(8); // name="T"
    pub const V: GraphemeClusterBreak = GraphemeClusterBreak(9); // name="V"
    pub const SpacingMark: GraphemeClusterBreak = GraphemeClusterBreak(10); // name="SM"
    pub const Prepend: GraphemeClusterBreak = GraphemeClusterBreak(11); // name="PP"
    pub const RegionalIndicator: GraphemeClusterBreak = GraphemeClusterBreak(12); // name="RI"
    /// This value is obsolete and unused.
    pub const EBase: GraphemeClusterBreak = GraphemeClusterBreak(13); // name="EB"
    /// This value is obsolete and unused.
    pub const EBaseGAZ: GraphemeClusterBreak = GraphemeClusterBreak(14); // name="EBG"
    /// This value is obsolete and unused.
    pub const EModifier: GraphemeClusterBreak = GraphemeClusterBreak(15); // name="EM"
    /// This value is obsolete and unused.
    pub const GlueAfterZwj: GraphemeClusterBreak = GraphemeClusterBreak(16); // name="GAZ"
    pub const ZWJ: GraphemeClusterBreak = GraphemeClusterBreak(17); // name="ZWJ"
}
}

make_enumerated_property! {
    name: "Grapheme_Cluster_Break";
    short_name: "GCB";
    ident: GraphemeClusterBreak;
    data_marker: crate::provider::PropertyEnumGraphemeClusterBreakV1;
    singleton: SINGLETON_PROPERTY_ENUM_GRAPHEME_CLUSTER_BREAK_V1;
    ule_ty: u8;
}

/// Enumerated property Word_Break.
///
/// See "Default Word Boundary Specification" in UAX #29 for the summary of
/// each property value:
/// <https://www.unicode.org/reports/tr29/#Default_Word_Boundaries>.
///
/// **Note:** Use `icu::segmenter` for an all-in-one break iterator implementation.
///
/// # Example
///
/// ```
/// use icu::properties::{props::WordBreak, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<WordBreak>::new().get('.'),
///     WordBreak::MidNumLet
/// ); // U+002E: Full Stop
/// assert_eq!(
///     CodePointMapData::<WordBreak>::new().get('，'),
///     WordBreak::MidNum
/// ); // U+FF0C: Fullwidth Comma
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct WordBreak(pub(crate) u8);

impl WordBreak {
    /// Returns an ICU4C `UWordBreak` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `UWordBreak` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
impl WordBreak {
    pub const Other: WordBreak = WordBreak(0); // name="XX"
    pub const ALetter: WordBreak = WordBreak(1); // name="LE"
    pub const Format: WordBreak = WordBreak(2); // name="FO"
    pub const Katakana: WordBreak = WordBreak(3); // name="KA"
    pub const MidLetter: WordBreak = WordBreak(4); // name="ML"
    pub const MidNum: WordBreak = WordBreak(5); // name="MN"
    pub const Numeric: WordBreak = WordBreak(6); // name="NU"
    pub const ExtendNumLet: WordBreak = WordBreak(7); // name="EX"
    pub const CR: WordBreak = WordBreak(8); // name="CR"
    pub const Extend: WordBreak = WordBreak(9); // name="Extend"
    pub const LF: WordBreak = WordBreak(10); // name="LF"
    pub const MidNumLet: WordBreak = WordBreak(11); // name="MB"
    pub const Newline: WordBreak = WordBreak(12); // name="NL"
    pub const RegionalIndicator: WordBreak = WordBreak(13); // name="RI"
    pub const HebrewLetter: WordBreak = WordBreak(14); // name="HL"
    pub const SingleQuote: WordBreak = WordBreak(15); // name="SQ"
    pub const DoubleQuote: WordBreak = WordBreak(16); // name=DQ
    /// This value is obsolete and unused.
    pub const EBase: WordBreak = WordBreak(17); // name="EB"
    /// This value is obsolete and unused.
    pub const EBaseGAZ: WordBreak = WordBreak(18); // name="EBG"
    /// This value is obsolete and unused.
    pub const EModifier: WordBreak = WordBreak(19); // name="EM"
    /// This value is obsolete and unused.
    pub const GlueAfterZwj: WordBreak = WordBreak(20); // name="GAZ"
    pub const ZWJ: WordBreak = WordBreak(21); // name="ZWJ"
    pub const WSegSpace: WordBreak = WordBreak(22); // name="WSegSpace"
}
}

make_enumerated_property! {
    name: "Word_Break";
    short_name: "WB";
    ident: WordBreak;
    data_marker: crate::provider::PropertyEnumWordBreakV1;
    singleton: SINGLETON_PROPERTY_ENUM_WORD_BREAK_V1;
    ule_ty: u8;
}

/// Enumerated property Sentence_Break.
///
/// See "Default Sentence Boundary Specification" in UAX #29 for the summary of
/// each property value:
/// <https://www.unicode.org/reports/tr29/#Default_Word_Boundaries>.
///
/// **Note:** Use `icu::segmenter` for an all-in-one break iterator implementation.
///
/// # Example
///
/// ```
/// use icu::properties::{props::SentenceBreak, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<SentenceBreak>::new().get('９'),
///     SentenceBreak::Numeric
/// ); // U+FF19: Fullwidth Digit Nine
/// assert_eq!(
///     CodePointMapData::<SentenceBreak>::new().get(','),
///     SentenceBreak::SContinue
/// ); // U+002C: Comma
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct SentenceBreak(pub(crate) u8);

impl SentenceBreak {
    /// Returns an ICU4C `USentenceBreak` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `USentenceBreak` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
impl SentenceBreak {
    pub const Other: SentenceBreak = SentenceBreak(0); // name="XX"
    pub const ATerm: SentenceBreak = SentenceBreak(1); // name="AT"
    pub const Close: SentenceBreak = SentenceBreak(2); // name="CL"
    pub const Format: SentenceBreak = SentenceBreak(3); // name="FO"
    pub const Lower: SentenceBreak = SentenceBreak(4); // name="LO"
    pub const Numeric: SentenceBreak = SentenceBreak(5); // name="NU"
    pub const OLetter: SentenceBreak = SentenceBreak(6); // name="LE"
    pub const Sep: SentenceBreak = SentenceBreak(7); // name="SE"
    pub const Sp: SentenceBreak = SentenceBreak(8); // name="SP"
    pub const STerm: SentenceBreak = SentenceBreak(9); // name="ST"
    pub const Upper: SentenceBreak = SentenceBreak(10); // name="UP"
    pub const CR: SentenceBreak = SentenceBreak(11); // name="CR"
    pub const Extend: SentenceBreak = SentenceBreak(12); // name="EX"
    pub const LF: SentenceBreak = SentenceBreak(13); // name="LF"
    pub const SContinue: SentenceBreak = SentenceBreak(14); // name="SC"
}
}

make_enumerated_property! {
    name: "Sentence_Break";
    short_name: "SB";
    ident: SentenceBreak;
    data_marker: crate::provider::PropertyEnumSentenceBreakV1;
    singleton: SINGLETON_PROPERTY_ENUM_SENTENCE_BREAK_V1;
    ule_ty: u8;
}

/// Property Canonical_Combining_Class.
/// See UAX #15:
/// <https://www.unicode.org/reports/tr15/>.
///
/// See `icu::normalizer::properties::CanonicalCombiningClassMap` for the API
/// to look up the Canonical_Combining_Class property by scalar value.
///
/// **Note:** See `icu::normalizer::CanonicalCombiningClassMap` for the preferred API
/// to look up the Canonical_Combining_Class property by scalar value.
///
/// # Example
///
/// ```
/// use icu::properties::{props::CanonicalCombiningClass, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<CanonicalCombiningClass>::new().get('a'),
///     CanonicalCombiningClass::NotReordered
/// ); // U+0061: LATIN SMALL LETTER A
/// assert_eq!(
///     CodePointMapData::<CanonicalCombiningClass>::new().get('\u{0301}'),
///     CanonicalCombiningClass::Above
/// ); // U+0301: COMBINING ACUTE ACCENT
/// ```
//
// NOTE: The Pernosco debugger has special knowledge
// of this struct. Please do not change the bit layout
// or the crate-module-qualified name of this struct
// without coordination.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct CanonicalCombiningClass(pub(crate) u8);

impl CanonicalCombiningClass {
    /// Returns an ICU4C `UCanonicalCombiningClass` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `UCanonicalCombiningClass` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
// These constant names come from PropertyValueAliases.txt
#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
impl CanonicalCombiningClass {
    pub const NotReordered: CanonicalCombiningClass = CanonicalCombiningClass(0); // name="NR"
    pub const Overlay: CanonicalCombiningClass = CanonicalCombiningClass(1); // name="OV"
    pub const HanReading: CanonicalCombiningClass = CanonicalCombiningClass(6); // name="HANR"
    pub const Nukta: CanonicalCombiningClass = CanonicalCombiningClass(7); // name="NK"
    pub const KanaVoicing: CanonicalCombiningClass = CanonicalCombiningClass(8); // name="KV"
    pub const Virama: CanonicalCombiningClass = CanonicalCombiningClass(9); // name="VR"
    pub const CCC10: CanonicalCombiningClass = CanonicalCombiningClass(10); // name="CCC10"
    pub const CCC11: CanonicalCombiningClass = CanonicalCombiningClass(11); // name="CCC11"
    pub const CCC12: CanonicalCombiningClass = CanonicalCombiningClass(12); // name="CCC12"
    pub const CCC13: CanonicalCombiningClass = CanonicalCombiningClass(13); // name="CCC13"
    pub const CCC14: CanonicalCombiningClass = CanonicalCombiningClass(14); // name="CCC14"
    pub const CCC15: CanonicalCombiningClass = CanonicalCombiningClass(15); // name="CCC15"
    pub const CCC16: CanonicalCombiningClass = CanonicalCombiningClass(16); // name="CCC16"
    pub const CCC17: CanonicalCombiningClass = CanonicalCombiningClass(17); // name="CCC17"
    pub const CCC18: CanonicalCombiningClass = CanonicalCombiningClass(18); // name="CCC18"
    pub const CCC19: CanonicalCombiningClass = CanonicalCombiningClass(19); // name="CCC19"
    pub const CCC20: CanonicalCombiningClass = CanonicalCombiningClass(20); // name="CCC20"
    pub const CCC21: CanonicalCombiningClass = CanonicalCombiningClass(21); // name="CCC21"
    pub const CCC22: CanonicalCombiningClass = CanonicalCombiningClass(22); // name="CCC22"
    pub const CCC23: CanonicalCombiningClass = CanonicalCombiningClass(23); // name="CCC23"
    pub const CCC24: CanonicalCombiningClass = CanonicalCombiningClass(24); // name="CCC24"
    pub const CCC25: CanonicalCombiningClass = CanonicalCombiningClass(25); // name="CCC25"
    pub const CCC26: CanonicalCombiningClass = CanonicalCombiningClass(26); // name="CCC26"
    pub const CCC27: CanonicalCombiningClass = CanonicalCombiningClass(27); // name="CCC27"
    pub const CCC28: CanonicalCombiningClass = CanonicalCombiningClass(28); // name="CCC28"
    pub const CCC29: CanonicalCombiningClass = CanonicalCombiningClass(29); // name="CCC29"
    pub const CCC30: CanonicalCombiningClass = CanonicalCombiningClass(30); // name="CCC30"
    pub const CCC31: CanonicalCombiningClass = CanonicalCombiningClass(31); // name="CCC31"
    pub const CCC32: CanonicalCombiningClass = CanonicalCombiningClass(32); // name="CCC32"
    pub const CCC33: CanonicalCombiningClass = CanonicalCombiningClass(33); // name="CCC33"
    pub const CCC34: CanonicalCombiningClass = CanonicalCombiningClass(34); // name="CCC34"
    pub const CCC35: CanonicalCombiningClass = CanonicalCombiningClass(35); // name="CCC35"
    pub const CCC36: CanonicalCombiningClass = CanonicalCombiningClass(36); // name="CCC36"
    pub const CCC84: CanonicalCombiningClass = CanonicalCombiningClass(84); // name="CCC84"
    pub const CCC91: CanonicalCombiningClass = CanonicalCombiningClass(91); // name="CCC91"
    pub const CCC103: CanonicalCombiningClass = CanonicalCombiningClass(103); // name="CCC103"
    pub const CCC107: CanonicalCombiningClass = CanonicalCombiningClass(107); // name="CCC107"
    pub const CCC118: CanonicalCombiningClass = CanonicalCombiningClass(118); // name="CCC118"
    pub const CCC122: CanonicalCombiningClass = CanonicalCombiningClass(122); // name="CCC122"
    pub const CCC129: CanonicalCombiningClass = CanonicalCombiningClass(129); // name="CCC129"
    pub const CCC130: CanonicalCombiningClass = CanonicalCombiningClass(130); // name="CCC130"
    pub const CCC132: CanonicalCombiningClass = CanonicalCombiningClass(132); // name="CCC132"
    pub const CCC133: CanonicalCombiningClass = CanonicalCombiningClass(133); // name="CCC133" // RESERVED
    pub const AttachedBelowLeft: CanonicalCombiningClass = CanonicalCombiningClass(200); // name="ATBL"
    pub const AttachedBelow: CanonicalCombiningClass = CanonicalCombiningClass(202); // name="ATB"
    pub const AttachedAbove: CanonicalCombiningClass = CanonicalCombiningClass(214); // name="ATA"
    pub const AttachedAboveRight: CanonicalCombiningClass = CanonicalCombiningClass(216); // name="ATAR"
    pub const BelowLeft: CanonicalCombiningClass = CanonicalCombiningClass(218); // name="BL"
    pub const Below: CanonicalCombiningClass = CanonicalCombiningClass(220); // name="B"
    pub const BelowRight: CanonicalCombiningClass = CanonicalCombiningClass(222); // name="BR"
    pub const Left: CanonicalCombiningClass = CanonicalCombiningClass(224); // name="L"
    pub const Right: CanonicalCombiningClass = CanonicalCombiningClass(226); // name="R"
    pub const AboveLeft: CanonicalCombiningClass = CanonicalCombiningClass(228); // name="AL"
    pub const Above: CanonicalCombiningClass = CanonicalCombiningClass(230); // name="A"
    pub const AboveRight: CanonicalCombiningClass = CanonicalCombiningClass(232); // name="AR"
    pub const DoubleBelow: CanonicalCombiningClass = CanonicalCombiningClass(233); // name="DB"
    pub const DoubleAbove: CanonicalCombiningClass = CanonicalCombiningClass(234); // name="DA"
    pub const IotaSubscript: CanonicalCombiningClass = CanonicalCombiningClass(240); // name="IS"
}
}

make_enumerated_property! {
    name: "Canonical_Combining_Class";
    short_name: "ccc";
    ident: CanonicalCombiningClass;
    data_marker: crate::provider::PropertyEnumCanonicalCombiningClassV1;
    singleton: SINGLETON_PROPERTY_ENUM_CANONICAL_COMBINING_CLASS_V1;
    ule_ty: u8;
}

/// Property Indic_Conjunct_Break.
/// See UAX #44:
/// <https://www.unicode.org/reports/tr44/#Indic_Conjunct_Break>.
///
/// # Example
///
/// ```
/// use icu::properties::{props::IndicConjunctBreak, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<IndicConjunctBreak>::new().get('a'),
///     IndicConjunctBreak::None
/// );
/// assert_eq!(
///     CodePointMapData::<IndicConjunctBreak>::new().get('\u{094d}'),
///     IndicConjunctBreak::Linker
/// );
/// assert_eq!(
///     CodePointMapData::<IndicConjunctBreak>::new().get('\u{0915}'),
///     IndicConjunctBreak::Consonant
/// );
/// assert_eq!(
///     CodePointMapData::<IndicConjunctBreak>::new().get('\u{0300}'),
///     IndicConjunctBreak::Extend
/// );
/// ```
#[doc(hidden)] // draft API in ICU4C
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct IndicConjunctBreak(pub(crate) u8);

impl IndicConjunctBreak {
    /// Returns an ICU4C `UIndicConjunctBreak` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `UIndicConjunctBreak` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[doc(hidden)] // draft API in ICU4C
#[allow(non_upper_case_globals)]
impl IndicConjunctBreak {
    pub const None: IndicConjunctBreak = IndicConjunctBreak(0);
    pub const Consonant: IndicConjunctBreak = IndicConjunctBreak(1);
    pub const Extend: IndicConjunctBreak = IndicConjunctBreak(2);
    pub const Linker: IndicConjunctBreak = IndicConjunctBreak(3);
}
}

make_enumerated_property! {
    name: "Indic_Conjunct_Break";
    short_name: "InCB";
    ident: IndicConjunctBreak;
    data_marker: crate::provider::PropertyEnumIndicConjunctBreakV1;
    singleton: SINGLETON_PROPERTY_ENUM_INDIC_CONJUNCT_BREAK_V1;
    ule_ty: u8;
}

/// Property Indic_Syllabic_Category.
/// See UAX #44:
/// <https://www.unicode.org/reports/tr44/#Indic_Syllabic_Category>.
///
/// # Example
///
/// ```
/// use icu::properties::{props::IndicSyllabicCategory, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<IndicSyllabicCategory>::new().get('a'),
///     IndicSyllabicCategory::Other
/// );
/// assert_eq!(
///     CodePointMapData::<IndicSyllabicCategory>::new().get('\u{0900}'),
///     IndicSyllabicCategory::Bindu
/// ); // U+0900: DEVANAGARI SIGN INVERTED CANDRABINDU
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct IndicSyllabicCategory(pub(crate) u8);

impl IndicSyllabicCategory {
    /// Returns an ICU4C `UIndicSyllabicCategory` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `UIndicSyllabicCategory` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
impl IndicSyllabicCategory {
    pub const Other: IndicSyllabicCategory = IndicSyllabicCategory(0);
    pub const Avagraha: IndicSyllabicCategory = IndicSyllabicCategory(1);
    pub const Bindu: IndicSyllabicCategory = IndicSyllabicCategory(2);
    pub const BrahmiJoiningNumber: IndicSyllabicCategory = IndicSyllabicCategory(3);
    pub const CantillationMark: IndicSyllabicCategory = IndicSyllabicCategory(4);
    pub const Consonant: IndicSyllabicCategory = IndicSyllabicCategory(5);
    pub const ConsonantDead: IndicSyllabicCategory = IndicSyllabicCategory(6);
    pub const ConsonantFinal: IndicSyllabicCategory = IndicSyllabicCategory(7);
    pub const ConsonantHeadLetter: IndicSyllabicCategory = IndicSyllabicCategory(8);
    pub const ConsonantInitialPostfixed: IndicSyllabicCategory = IndicSyllabicCategory(9);
    pub const ConsonantKiller: IndicSyllabicCategory = IndicSyllabicCategory(10);
    pub const ConsonantMedial: IndicSyllabicCategory = IndicSyllabicCategory(11);
    pub const ConsonantPlaceholder: IndicSyllabicCategory = IndicSyllabicCategory(12);
    pub const ConsonantPrecedingRepha: IndicSyllabicCategory = IndicSyllabicCategory(13);
    pub const ConsonantPrefixed: IndicSyllabicCategory = IndicSyllabicCategory(14);
    pub const ConsonantSucceedingRepha: IndicSyllabicCategory = IndicSyllabicCategory(15);
    pub const ConsonantSubjoined: IndicSyllabicCategory = IndicSyllabicCategory(16);
    pub const ConsonantWithStacker: IndicSyllabicCategory = IndicSyllabicCategory(17);
    pub const GeminationMark: IndicSyllabicCategory = IndicSyllabicCategory(18);
    pub const InvisibleStacker: IndicSyllabicCategory = IndicSyllabicCategory(19);
    pub const Joiner: IndicSyllabicCategory = IndicSyllabicCategory(20);
    pub const ModifyingLetter: IndicSyllabicCategory = IndicSyllabicCategory(21);
    pub const NonJoiner: IndicSyllabicCategory = IndicSyllabicCategory(22);
    pub const Nukta: IndicSyllabicCategory = IndicSyllabicCategory(23);
    pub const Number: IndicSyllabicCategory = IndicSyllabicCategory(24);
    pub const NumberJoiner: IndicSyllabicCategory = IndicSyllabicCategory(25);
    pub const PureKiller: IndicSyllabicCategory = IndicSyllabicCategory(26);
    pub const RegisterShifter: IndicSyllabicCategory = IndicSyllabicCategory(27);
    pub const SyllableModifier: IndicSyllabicCategory = IndicSyllabicCategory(28);
    pub const ToneLetter: IndicSyllabicCategory = IndicSyllabicCategory(29);
    pub const ToneMark: IndicSyllabicCategory = IndicSyllabicCategory(30);
    pub const Virama: IndicSyllabicCategory = IndicSyllabicCategory(31);
    pub const Visarga: IndicSyllabicCategory = IndicSyllabicCategory(32);
    pub const Vowel: IndicSyllabicCategory = IndicSyllabicCategory(33);
    pub const VowelDependent: IndicSyllabicCategory = IndicSyllabicCategory(34);
    pub const VowelIndependent: IndicSyllabicCategory = IndicSyllabicCategory(35);
    pub const ReorderingKiller: IndicSyllabicCategory = IndicSyllabicCategory(36);
}
}

make_enumerated_property! {
    name: "Indic_Syllabic_Category";
    short_name: "InSC";
    ident: IndicSyllabicCategory;
    data_marker: crate::provider::PropertyEnumIndicSyllabicCategoryV1;
    singleton: SINGLETON_PROPERTY_ENUM_INDIC_SYLLABIC_CATEGORY_V1;
    ule_ty: u8;
}

/// Enumerated property Joining_Type.
///
/// See Section 9.2, Arabic Cursive Joining in The Unicode Standard for the summary of
/// each property value.
///
/// # Example
///
/// ```
/// use icu::properties::{props::JoiningType, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<JoiningType>::new().get('ؠ'),
///     JoiningType::DualJoining
/// ); // U+0620: Arabic Letter Kashmiri Yeh
/// assert_eq!(
///     CodePointMapData::<JoiningType>::new().get('𐫍'),
///     JoiningType::LeftJoining
/// ); // U+10ACD: Manichaean Letter Heth
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct JoiningType(pub(crate) u8);

impl JoiningType {
    /// Returns an ICU4C `UJoiningType` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `UJoiningType` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
impl JoiningType {
    pub const NonJoining: JoiningType = JoiningType(0); // name="U"
    pub const JoinCausing: JoiningType = JoiningType(1); // name="C"
    pub const DualJoining: JoiningType = JoiningType(2); // name="D"
    pub const LeftJoining: JoiningType = JoiningType(3); // name="L"
    pub const RightJoining: JoiningType = JoiningType(4); // name="R"
    pub const Transparent: JoiningType = JoiningType(5); // name="T"
}
}

make_enumerated_property! {
    name: "Joining_Type";
    short_name: "jt";
    ident: JoiningType;
    data_marker: crate::provider::PropertyEnumJoiningTypeV1;
    singleton: SINGLETON_PROPERTY_ENUM_JOINING_TYPE_V1;
    ule_ty: u8;
}

/// Property Vertical_Orientation
///
/// See UTR #50:
/// <https://www.unicode.org/reports/tr50/#vo>
///
/// # Example
///
/// ```
/// use icu::properties::{props::VerticalOrientation, CodePointMapData};
///
/// assert_eq!(
///     CodePointMapData::<VerticalOrientation>::new().get('a'),
///     VerticalOrientation::Rotated
/// );
/// assert_eq!(
///     CodePointMapData::<VerticalOrientation>::new().get('§'),
///     VerticalOrientation::Upright
/// );
/// assert_eq!(
///     CodePointMapData::<VerticalOrientation>::new().get32(0x2329),
///     VerticalOrientation::TransformedRotated
/// );
/// assert_eq!(
///     CodePointMapData::<VerticalOrientation>::new().get32(0x3001),
///     VerticalOrientation::TransformedUpright
/// );
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // newtype
#[repr(transparent)]
pub struct VerticalOrientation(pub(crate) u8);

impl VerticalOrientation {
    /// Returns an ICU4C `UVerticalOrientation` value.
    pub const fn to_icu4c_value(self) -> u8 {
        self.0
    }
    /// Constructor from an ICU4C `UVerticalOrientation` value.
    pub const fn from_icu4c_value(value: u8) -> Self {
        Self(value)
    }
}

create_const_array! {
#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
impl VerticalOrientation {
    pub const Rotated: VerticalOrientation = VerticalOrientation(0); // name="R"
    pub const TransformedRotated: VerticalOrientation = VerticalOrientation(1); // name="Tr"
    pub const TransformedUpright: VerticalOrientation = VerticalOrientation(2); // name="Tu"
    pub const Upright: VerticalOrientation = VerticalOrientation(3); // name="U"
}
}

make_enumerated_property! {
    name: "Vertical_Orientation";
    short_name: "vo";
    ident: VerticalOrientation;
    data_marker: crate::provider::PropertyEnumVerticalOrientationV1;
    singleton: SINGLETON_PROPERTY_ENUM_VERTICAL_ORIENTATION_V1;
    ule_ty: u8;
}

pub use crate::code_point_set::BinaryProperty;

macro_rules! make_binary_property {
    (
        name: $name:literal;
        short_name: $short_name:literal;
        ident: $ident:ident;
        data_marker: $data_marker:ty;
        singleton: $singleton:ident;
            $(#[$doc:meta])+
    ) => {
        $(#[$doc])+
        #[derive(Debug)]
        #[non_exhaustive]
        pub struct $ident;

        impl crate::private::Sealed for $ident {}

        impl BinaryProperty for $ident {
        type DataMarker = $data_marker;
            #[cfg(feature = "compiled_data")]
            const SINGLETON: &'static crate::provider::PropertyCodePointSet<'static> =
                &crate::provider::Baked::$singleton;
            const NAME: &'static [u8] = $name.as_bytes();
            const SHORT_NAME: &'static [u8] = $short_name.as_bytes();
        }
    };
}

make_binary_property! {
    name: "ASCII_Hex_Digit";
    short_name: "AHex";
    ident: AsciiHexDigit;
    data_marker: crate::provider::PropertyBinaryAsciiHexDigitV1;
    singleton: SINGLETON_PROPERTY_BINARY_ASCII_HEX_DIGIT_V1;
    /// ASCII characters commonly used for the representation of hexadecimal numbers.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::AsciiHexDigit;
    ///
    /// let ascii_hex_digit = CodePointSetData::new::<AsciiHexDigit>();
    ///
    /// assert!(ascii_hex_digit.contains('3'));
    /// assert!(!ascii_hex_digit.contains('੩'));  // U+0A69 GURMUKHI DIGIT THREE
    /// assert!(ascii_hex_digit.contains('A'));
    /// assert!(!ascii_hex_digit.contains('Ä'));  // U+00C4 LATIN CAPITAL LETTER A WITH DIAERESIS
    /// ```
}

make_binary_property! {
    name: "Alnum";
    short_name: "Alnum";
    ident: Alnum;
    data_marker: crate::provider::PropertyBinaryAlnumV1;
    singleton: SINGLETON_PROPERTY_BINARY_ALNUM_V1;
    /// Characters with the `Alphabetic` or `Decimal_Number` property.
    ///
    /// This is defined for POSIX compatibility.
}

make_binary_property! {
    name: "Alphabetic";
    short_name: "Alpha";
    ident: Alphabetic;
    data_marker: crate::provider::PropertyBinaryAlphabeticV1;
    singleton: SINGLETON_PROPERTY_BINARY_ALPHABETIC_V1;
    /// Alphabetic characters.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Alphabetic;
    ///
    /// let alphabetic = CodePointSetData::new::<Alphabetic>();
    ///
    /// assert!(!alphabetic.contains('3'));
    /// assert!(!alphabetic.contains('੩'));  // U+0A69 GURMUKHI DIGIT THREE
    /// assert!(alphabetic.contains('A'));
    /// assert!(alphabetic.contains('Ä'));  // U+00C4 LATIN CAPITAL LETTER A WITH DIAERESIS
    /// ```

}

make_binary_property! {
    name: "Bidi_Control";
    short_name: "Bidi_C";
    ident: BidiControl;
    data_marker: crate::provider::PropertyBinaryBidiControlV1;
    singleton: SINGLETON_PROPERTY_BINARY_BIDI_CONTROL_V1;
    /// Format control characters which have specific functions in the Unicode Bidirectional
    /// Algorithm.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::BidiControl;
    ///
    /// let bidi_control = CodePointSetData::new::<BidiControl>();
    ///
    /// assert!(bidi_control.contains('\u{200F}'));  // RIGHT-TO-LEFT MARK
    /// assert!(!bidi_control.contains('ش'));  // U+0634 ARABIC LETTER SHEEN
    /// ```

}

make_binary_property! {
    name: "Bidi_Mirrored";
    short_name: "Bidi_M";
    ident: BidiMirrored;
    data_marker: crate::provider::PropertyBinaryBidiMirroredV1;
    singleton: SINGLETON_PROPERTY_BINARY_BIDI_MIRRORED_V1;
    /// Characters that are mirrored in bidirectional text.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::BidiMirrored;
    ///
    /// let bidi_mirrored = CodePointSetData::new::<BidiMirrored>();
    ///
    /// assert!(bidi_mirrored.contains('['));
    /// assert!(bidi_mirrored.contains(']'));
    /// assert!(bidi_mirrored.contains('∑'));  // U+2211 N-ARY SUMMATION
    /// assert!(!bidi_mirrored.contains('ཉ'));  // U+0F49 TIBETAN LETTER NYA
    /// ```

}

make_binary_property! {
    name: "Blank";
    short_name: "Blank";
    ident: Blank;
    data_marker: crate::provider::PropertyBinaryBlankV1;
    singleton: SINGLETON_PROPERTY_BINARY_BLANK_V1;
    /// Horizontal whitespace characters

}

make_binary_property! {
    name: "Cased";
    short_name: "Cased";
    ident: Cased;
    data_marker: crate::provider::PropertyBinaryCasedV1;
    singleton: SINGLETON_PROPERTY_BINARY_CASED_V1;
    /// Uppercase, lowercase, and titlecase characters.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Cased;
    ///
    /// let cased = CodePointSetData::new::<Cased>();
    ///
    /// assert!(cased.contains('Ꙡ'));  // U+A660 CYRILLIC CAPITAL LETTER REVERSED TSE
    /// assert!(!cased.contains('ދ'));  // U+078B THAANA LETTER DHAALU
    /// ```

}

make_binary_property! {
    name: "Case_Ignorable";
    short_name: "CI";
    ident: CaseIgnorable;
    data_marker: crate::provider::PropertyBinaryCaseIgnorableV1;
    singleton: SINGLETON_PROPERTY_BINARY_CASE_IGNORABLE_V1;
    /// Characters which are ignored for casing purposes.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::CaseIgnorable;
    ///
    /// let case_ignorable = CodePointSetData::new::<CaseIgnorable>();
    ///
    /// assert!(case_ignorable.contains(':'));
    /// assert!(!case_ignorable.contains('λ'));  // U+03BB GREEK SMALL LETTER LAMBDA
    /// ```

}

make_binary_property! {
    name: "Full_Composition_Exclusion";
    short_name: "Comp_Ex";
    ident: FullCompositionExclusion;
    data_marker: crate::provider::PropertyBinaryFullCompositionExclusionV1;
    singleton: SINGLETON_PROPERTY_BINARY_FULL_COMPOSITION_EXCLUSION_V1;
    /// Characters that are excluded from composition.
    ///
    /// See <https://unicode.org/Public/UNIDATA/CompositionExclusions.txt>

}

make_binary_property! {
    name: "Changes_When_Casefolded";
    short_name: "CWCF";
    ident: ChangesWhenCasefolded;
    data_marker: crate::provider::PropertyBinaryChangesWhenCasefoldedV1;
    singleton: SINGLETON_PROPERTY_BINARY_CHANGES_WHEN_CASEFOLDED_V1;
    /// Characters whose normalized forms are not stable under case folding.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::ChangesWhenCasefolded;
    ///
    /// let changes_when_casefolded = CodePointSetData::new::<ChangesWhenCasefolded>();
    ///
    /// assert!(changes_when_casefolded.contains('ß'));  // U+00DF LATIN SMALL LETTER SHARP S
    /// assert!(!changes_when_casefolded.contains('ᜉ'));  // U+1709 TAGALOG LETTER PA
    /// ```

}

make_binary_property! {
    name: "Changes_When_Casemapped";
    short_name: "CWCM";
    ident: ChangesWhenCasemapped;
    data_marker: crate::provider::PropertyBinaryChangesWhenCasemappedV1;
    singleton: SINGLETON_PROPERTY_BINARY_CHANGES_WHEN_CASEMAPPED_V1;
    /// Characters which may change when they undergo case mapping.

}

make_binary_property! {
    name: "Changes_When_NFKC_Casefolded";
    short_name: "CWKCF";
    ident: ChangesWhenNfkcCasefolded;
    data_marker: crate::provider::PropertyBinaryChangesWhenNfkcCasefoldedV1;
    singleton: SINGLETON_PROPERTY_BINARY_CHANGES_WHEN_NFKC_CASEFOLDED_V1;
    /// Characters which are not identical to their `NFKC_Casefold` mapping.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::ChangesWhenNfkcCasefolded;
    ///
    /// let changes_when_nfkc_casefolded = CodePointSetData::new::<ChangesWhenNfkcCasefolded>();
    ///
    /// assert!(changes_when_nfkc_casefolded.contains('🄵'));  // U+1F135 SQUARED LATIN CAPITAL LETTER F
    /// assert!(!changes_when_nfkc_casefolded.contains('f'));
    /// ```

}

make_binary_property! {
    name: "Changes_When_Lowercased";
    short_name: "CWL";
    ident: ChangesWhenLowercased;
    data_marker: crate::provider::PropertyBinaryChangesWhenLowercasedV1;
    singleton: SINGLETON_PROPERTY_BINARY_CHANGES_WHEN_LOWERCASED_V1;
    /// Characters whose normalized forms are not stable under a `toLowercase` mapping.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::ChangesWhenLowercased;
    ///
    /// let changes_when_lowercased = CodePointSetData::new::<ChangesWhenLowercased>();
    ///
    /// assert!(changes_when_lowercased.contains('Ⴔ'));  // U+10B4 GEORGIAN CAPITAL LETTER PHAR
    /// assert!(!changes_when_lowercased.contains('ფ'));  // U+10E4 GEORGIAN LETTER PHAR
    /// ```

}

make_binary_property! {
    name: "Changes_When_Titlecased";
    short_name: "CWT";
    ident: ChangesWhenTitlecased;
    data_marker: crate::provider::PropertyBinaryChangesWhenTitlecasedV1;
    singleton: SINGLETON_PROPERTY_BINARY_CHANGES_WHEN_TITLECASED_V1;
    /// Characters whose normalized forms are not stable under a `toTitlecase` mapping.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::ChangesWhenTitlecased;
    ///
    /// let changes_when_titlecased = CodePointSetData::new::<ChangesWhenTitlecased>();
    ///
    /// assert!(changes_when_titlecased.contains('æ'));  // U+00E6 LATIN SMALL LETTER AE
    /// assert!(!changes_when_titlecased.contains('Æ'));  // U+00E6 LATIN CAPITAL LETTER AE
    /// ```

}

make_binary_property! {
    name: "Changes_When_Uppercased";
    short_name: "CWU";
    ident: ChangesWhenUppercased;
    data_marker: crate::provider::PropertyBinaryChangesWhenUppercasedV1;
    singleton: SINGLETON_PROPERTY_BINARY_CHANGES_WHEN_UPPERCASED_V1;
    /// Characters whose normalized forms are not stable under a `toUppercase` mapping.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::ChangesWhenUppercased;
    ///
    /// let changes_when_uppercased = CodePointSetData::new::<ChangesWhenUppercased>();
    ///
    /// assert!(changes_when_uppercased.contains('ւ'));  // U+0582 ARMENIAN SMALL LETTER YIWN
    /// assert!(!changes_when_uppercased.contains('Ւ'));  // U+0552 ARMENIAN CAPITAL LETTER YIWN
    /// ```

}

make_binary_property! {
    name: "Dash";
    short_name: "Dash";
    ident: Dash;
    data_marker: crate::provider::PropertyBinaryDashV1;
    singleton: SINGLETON_PROPERTY_BINARY_DASH_V1;
    /// Punctuation characters explicitly called out as dashes in the Unicode Standard, plus
    /// their compatibility equivalents.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Dash;
    ///
    /// let dash = CodePointSetData::new::<Dash>();
    ///
    /// assert!(dash.contains('⸺'));  // U+2E3A TWO-EM DASH
    /// assert!(dash.contains('-'));  // U+002D
    /// assert!(!dash.contains('='));  // U+003D
    /// ```

}

make_binary_property! {
    name: "Deprecated";
    short_name: "Dep";
    ident: Deprecated;
    data_marker: crate::provider::PropertyBinaryDeprecatedV1;
    singleton: SINGLETON_PROPERTY_BINARY_DEPRECATED_V1;
    /// Deprecated characters.
    ///
    /// No characters will ever be removed from the standard, but the
    /// usage of deprecated characters is strongly discouraged.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Deprecated;
    ///
    /// let deprecated = CodePointSetData::new::<Deprecated>();
    ///
    /// assert!(deprecated.contains('ឣ'));  // U+17A3 KHMER INDEPENDENT VOWEL QAQ
    /// assert!(!deprecated.contains('A'));
    /// ```

}

make_binary_property! {
    name: "Default_Ignorable_Code_Point";
    short_name: "DI";
    ident: DefaultIgnorableCodePoint;
    data_marker: crate::provider::PropertyBinaryDefaultIgnorableCodePointV1;
    singleton: SINGLETON_PROPERTY_BINARY_DEFAULT_IGNORABLE_CODE_POINT_V1;
    /// For programmatic determination of default ignorable code points.
    ///
    /// New characters that
    /// should be ignored in rendering (unless explicitly supported) will be assigned in these
    /// ranges, permitting programs to correctly handle the default rendering of such
    /// characters when not otherwise supported.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::DefaultIgnorableCodePoint;
    ///
    /// let default_ignorable_code_point = CodePointSetData::new::<DefaultIgnorableCodePoint>();
    ///
    /// assert!(default_ignorable_code_point.contains('\u{180B}'));  // MONGOLIAN FREE VARIATION SELECTOR ONE
    /// assert!(!default_ignorable_code_point.contains('E'));
    /// ```

}

make_binary_property! {
    name: "Diacritic";
    short_name: "Dia";
    ident: Diacritic;
    data_marker: crate::provider::PropertyBinaryDiacriticV1;
    singleton: SINGLETON_PROPERTY_BINARY_DIACRITIC_V1;
    /// Characters that linguistically modify the meaning of another character to which they apply.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Diacritic;
    ///
    /// let diacritic = CodePointSetData::new::<Diacritic>();
    ///
    /// assert!(diacritic.contains('\u{05B3}'));  // HEBREW POINT HATAF QAMATS
    /// assert!(!diacritic.contains('א'));  // U+05D0 HEBREW LETTER ALEF
    /// ```

}

make_binary_property! {
    name: "Emoji_Modifier_Base";
    short_name: "EBase";
    ident: EmojiModifierBase;
    data_marker: crate::provider::PropertyBinaryEmojiModifierBaseV1;
    singleton: SINGLETON_PROPERTY_BINARY_EMOJI_MODIFIER_BASE_V1;
    /// Characters that can serve as a base for emoji modifiers.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::EmojiModifierBase;
    ///
    /// let emoji_modifier_base = CodePointSetData::new::<EmojiModifierBase>();
    ///
    /// assert!(emoji_modifier_base.contains('✊'));  // U+270A RAISED FIST
    /// assert!(!emoji_modifier_base.contains('⛰'));  // U+26F0 MOUNTAIN
    /// ```

}

make_binary_property! {
    name: "Emoji_Component";
    short_name: "EComp";
    ident: EmojiComponent;
    data_marker: crate::provider::PropertyBinaryEmojiComponentV1;
    singleton: SINGLETON_PROPERTY_BINARY_EMOJI_COMPONENT_V1;
    /// Characters used in emoji sequences that normally do not appear on emoji keyboards as
    /// separate choices, such as base characters for emoji keycaps.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::EmojiComponent;
    ///
    /// let emoji_component = CodePointSetData::new::<EmojiComponent>();
    ///
    /// assert!(emoji_component.contains('🇹'));  // U+1F1F9 REGIONAL INDICATOR SYMBOL LETTER T
    /// assert!(emoji_component.contains('\u{20E3}'));  // COMBINING ENCLOSING KEYCAP
    /// assert!(emoji_component.contains('7'));
    /// assert!(!emoji_component.contains('T'));
    /// ```

}

make_binary_property! {
    name: "Emoji_Modifier";
    short_name: "EMod";
    ident: EmojiModifier;
    data_marker: crate::provider::PropertyBinaryEmojiModifierV1;
    singleton: SINGLETON_PROPERTY_BINARY_EMOJI_MODIFIER_V1;
    /// Characters that are emoji modifiers.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::EmojiModifier;
    ///
    /// let emoji_modifier = CodePointSetData::new::<EmojiModifier>();
    ///
    /// assert!(emoji_modifier.contains('\u{1F3FD}'));  // EMOJI MODIFIER FITZPATRICK TYPE-4
    /// assert!(!emoji_modifier.contains('\u{200C}'));  // ZERO WIDTH NON-JOINER
    /// ```

}

make_binary_property! {
    name: "Emoji";
    short_name: "Emoji";
    ident: Emoji;
    data_marker: crate::provider::PropertyBinaryEmojiV1;
    singleton: SINGLETON_PROPERTY_BINARY_EMOJI_V1;
    /// Characters that are emoji.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Emoji;
    ///
    /// let emoji = CodePointSetData::new::<Emoji>();
    ///
    /// assert!(emoji.contains('🔥'));  // U+1F525 FIRE
    /// assert!(!emoji.contains('V'));
    /// ```

}

make_binary_property! {
    name: "Emoji_Presentation";
    short_name: "EPres";
    ident: EmojiPresentation;
    data_marker: crate::provider::PropertyBinaryEmojiPresentationV1;
    singleton: SINGLETON_PROPERTY_BINARY_EMOJI_PRESENTATION_V1;
    /// Characters that have emoji presentation by default.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::EmojiPresentation;
    ///
    /// let emoji_presentation = CodePointSetData::new::<EmojiPresentation>();
    ///
    /// assert!(emoji_presentation.contains('🦬')); // U+1F9AC BISON
    /// assert!(!emoji_presentation.contains('♻'));  // U+267B BLACK UNIVERSAL RECYCLING SYMBOL
    /// ```

}

make_binary_property! {
    name: "Extender";
    short_name: "Ext";
    ident: Extender;
    data_marker: crate::provider::PropertyBinaryExtenderV1;
    singleton: SINGLETON_PROPERTY_BINARY_EXTENDER_V1;
    /// Characters whose principal function is to extend the value of a preceding alphabetic
    /// character or to extend the shape of adjacent characters.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Extender;
    ///
    /// let extender = CodePointSetData::new::<Extender>();
    ///
    /// assert!(extender.contains('ヾ'));  // U+30FE KATAKANA VOICED ITERATION MARK
    /// assert!(extender.contains('ー'));  // U+30FC KATAKANA-HIRAGANA PROLONGED SOUND MARK
    /// assert!(!extender.contains('・'));  // U+30FB KATAKANA MIDDLE DOT
    /// ```

}

make_binary_property! {
    name: "Extended_Pictographic";
    short_name: "ExtPict";
    ident: ExtendedPictographic;
    data_marker: crate::provider::PropertyBinaryExtendedPictographicV1;
    singleton: SINGLETON_PROPERTY_BINARY_EXTENDED_PICTOGRAPHIC_V1;
    /// Pictographic symbols, as well as reserved ranges in blocks largely associated with
    /// emoji characters
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::ExtendedPictographic;
    ///
    /// let extended_pictographic = CodePointSetData::new::<ExtendedPictographic>();
    ///
    /// assert!(extended_pictographic.contains('🥳')); // U+1F973 FACE WITH PARTY HORN AND PARTY HAT
    /// assert!(!extended_pictographic.contains('🇪'));  // U+1F1EA REGIONAL INDICATOR SYMBOL LETTER E
    /// ```

}

make_binary_property! {
    name: "Graph";
    short_name: "Graph";
    ident: Graph;
    data_marker: crate::provider::PropertyBinaryGraphV1;
    singleton: SINGLETON_PROPERTY_BINARY_GRAPH_V1;
    /// Invisible characters.
    ///
    /// This is defined for POSIX compatibility.

}

make_binary_property! {
    name: "Grapheme_Base";
    short_name: "Gr_Base";
    ident: GraphemeBase;
    data_marker: crate::provider::PropertyBinaryGraphemeBaseV1;
    singleton: SINGLETON_PROPERTY_BINARY_GRAPHEME_BASE_V1;
    /// Property used together with the definition of Standard Korean Syllable Block to define
    /// "Grapheme base".
    ///
    /// See D58 in Chapter 3, Conformance in the Unicode Standard.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::GraphemeBase;
    ///
    /// let grapheme_base = CodePointSetData::new::<GraphemeBase>();
    ///
    /// assert!(grapheme_base.contains('ക'));  // U+0D15 MALAYALAM LETTER KA
    /// assert!(grapheme_base.contains('\u{0D3F}'));  // U+0D3F MALAYALAM VOWEL SIGN I
    /// assert!(!grapheme_base.contains('\u{0D3E}'));  // U+0D3E MALAYALAM VOWEL SIGN AA
    /// ```

}

make_binary_property! {
    name: "Grapheme_Extend";
    short_name: "Gr_Ext";
    ident: GraphemeExtend;
    data_marker: crate::provider::PropertyBinaryGraphemeExtendV1;
    singleton: SINGLETON_PROPERTY_BINARY_GRAPHEME_EXTEND_V1;
    /// Property used to define "Grapheme extender".
    ///
    /// See D59 in Chapter 3, Conformance in the
    /// Unicode Standard.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::GraphemeExtend;
    ///
    /// let grapheme_extend = CodePointSetData::new::<GraphemeExtend>();
    ///
    /// assert!(!grapheme_extend.contains('ക'));  // U+0D15 MALAYALAM LETTER KA
    /// assert!(!grapheme_extend.contains('\u{0D3F}'));  // U+0D3F MALAYALAM VOWEL SIGN I
    /// assert!(grapheme_extend.contains('\u{0D3E}'));  // U+0D3E MALAYALAM VOWEL SIGN AA
    /// ```

}

make_binary_property! {
    name: "Grapheme_Link";
    short_name: "Gr_Link";
    ident: GraphemeLink;
    data_marker: crate::provider::PropertyBinaryGraphemeLinkV1;
    singleton: SINGLETON_PROPERTY_BINARY_GRAPHEME_LINK_V1;
    /// Deprecated property.
    ///
    /// Formerly proposed for programmatic determination of grapheme
    /// cluster boundaries.
}

make_binary_property! {
    name: "Hex_Digit";
    short_name: "Hex";
    ident: HexDigit;
    data_marker: crate::provider::PropertyBinaryHexDigitV1;
    singleton: SINGLETON_PROPERTY_BINARY_HEX_DIGIT_V1;
    /// Characters commonly used for the representation of hexadecimal numbers, plus their
    /// compatibility equivalents.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::HexDigit;
    ///
    /// let hex_digit = CodePointSetData::new::<HexDigit>();
    ///
    /// assert!(hex_digit.contains('0'));
    /// assert!(!hex_digit.contains('੩'));  // U+0A69 GURMUKHI DIGIT THREE
    /// assert!(hex_digit.contains('f'));
    /// assert!(hex_digit.contains('ｆ'));  // U+FF46 FULLWIDTH LATIN SMALL LETTER F
    /// assert!(hex_digit.contains('Ｆ'));  // U+FF26 FULLWIDTH LATIN CAPITAL LETTER F
    /// assert!(!hex_digit.contains('Ä'));  // U+00C4 LATIN CAPITAL LETTER A WITH DIAERESIS
    /// ```
}

make_binary_property! {
    name: "Hyphen";
    short_name: "Hyphen";
    ident: Hyphen;
    data_marker: crate::provider::PropertyBinaryHyphenV1;
    singleton: SINGLETON_PROPERTY_BINARY_HYPHEN_V1;
    /// Deprecated property.
    ///
    /// Dashes which are used to mark connections between pieces of
    /// words, plus the Katakana middle dot.
}

make_binary_property! {
    name: "ID_Compat_Math_Continue";
    short_name: "ID_Compat_Math_Continue";
    ident: IdCompatMathContinue;
    data_marker: crate::provider::PropertyBinaryIdCompatMathContinueV1;
    singleton: SINGLETON_PROPERTY_BINARY_ID_COMPAT_MATH_CONTINUE_V1;
    /// ID_Compat_Math_Continue Property
}

make_binary_property! {
    name: "ID_Compat_Math_Start";
    short_name: "ID_Compat_Math_Start";
    ident: IdCompatMathStart;
    data_marker: crate::provider::PropertyBinaryIdCompatMathStartV1;
    singleton: SINGLETON_PROPERTY_BINARY_ID_COMPAT_MATH_START_V1;
    /// ID_Compat_Math_Start Property
}

make_binary_property! {
    name: "Id_Continue";
    short_name: "IDC";
    ident: IdContinue;
    data_marker: crate::provider::PropertyBinaryIdContinueV1;
    singleton: SINGLETON_PROPERTY_BINARY_ID_CONTINUE_V1;
    /// Characters that can come after the first character in an identifier.
    ///
    /// If using NFKC to
    /// fold differences between characters, use [`XidContinue`] instead.  See
    /// [`Unicode Standard Annex #31`](https://www.unicode.org/reports/tr31/tr31-35.html) for
    /// more details.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::IdContinue;
    ///
    /// let id_continue = CodePointSetData::new::<IdContinue>();
    ///
    /// assert!(id_continue.contains('x'));
    /// assert!(id_continue.contains('1'));
    /// assert!(id_continue.contains('_'));
    /// assert!(id_continue.contains('ߝ'));  // U+07DD NKO LETTER FA
    /// assert!(!id_continue.contains('ⓧ'));  // U+24E7 CIRCLED LATIN SMALL LETTER X
    /// assert!(id_continue.contains('\u{FC5E}'));  // ARABIC LIGATURE SHADDA WITH DAMMATAN ISOLATED FORM
    /// ```
}

make_binary_property! {
    name: "Ideographic";
    short_name: "Ideo";
    ident: Ideographic;
    data_marker: crate::provider::PropertyBinaryIdeographicV1;
    singleton: SINGLETON_PROPERTY_BINARY_IDEOGRAPHIC_V1;
    /// Characters considered to be CJKV (Chinese, Japanese, Korean, and Vietnamese)
    /// ideographs, or related siniform ideographs
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Ideographic;
    ///
    /// let ideographic = CodePointSetData::new::<Ideographic>();
    ///
    /// assert!(ideographic.contains('川'));  // U+5DDD CJK UNIFIED IDEOGRAPH-5DDD
    /// assert!(!ideographic.contains('밥'));  // U+BC25 HANGUL SYLLABLE BAB
    /// ```
}

make_binary_property! {
    name: "Id_Start";
    short_name: "IDS";
    ident: IdStart;
    data_marker: crate::provider::PropertyBinaryIdStartV1;
    singleton: SINGLETON_PROPERTY_BINARY_ID_START_V1;
    /// Characters that can begin an identifier.
    ///
    /// If using NFKC to fold differences between
    /// characters, use [`XidStart`] instead.  See [`Unicode Standard Annex
    /// #31`](https://www.unicode.org/reports/tr31/tr31-35.html) for more details.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::IdStart;
    ///
    /// let id_start = CodePointSetData::new::<IdStart>();
    ///
    /// assert!(id_start.contains('x'));
    /// assert!(!id_start.contains('1'));
    /// assert!(!id_start.contains('_'));
    /// assert!(id_start.contains('ߝ'));  // U+07DD NKO LETTER FA
    /// assert!(!id_start.contains('ⓧ'));  // U+24E7 CIRCLED LATIN SMALL LETTER X
    /// assert!(id_start.contains('\u{FC5E}'));  // ARABIC LIGATURE SHADDA WITH DAMMATAN ISOLATED FORM
    /// ```
}

make_binary_property! {
    name: "Ids_Binary_Operator";
    short_name: "IDSB";
    ident: IdsBinaryOperator;
    data_marker: crate::provider::PropertyBinaryIdsBinaryOperatorV1;
    singleton: SINGLETON_PROPERTY_BINARY_IDS_BINARY_OPERATOR_V1;
    /// Characters used in Ideographic Description Sequences.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::IdsBinaryOperator;
    ///
    /// let ids_binary_operator = CodePointSetData::new::<IdsBinaryOperator>();
    ///
    /// assert!(ids_binary_operator.contains('\u{2FF5}'));  // IDEOGRAPHIC DESCRIPTION CHARACTER SURROUND FROM ABOVE
    /// assert!(!ids_binary_operator.contains('\u{3006}'));  // IDEOGRAPHIC CLOSING MARK
    /// ```
}

make_binary_property! {
    name: "Ids_Trinary_Operator";
    short_name: "IDST";
    ident: IdsTrinaryOperator;
    data_marker: crate::provider::PropertyBinaryIdsTrinaryOperatorV1;
    singleton: SINGLETON_PROPERTY_BINARY_IDS_TRINARY_OPERATOR_V1;
    /// Characters used in Ideographic Description Sequences.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::IdsTrinaryOperator;
    ///
    /// let ids_trinary_operator = CodePointSetData::new::<IdsTrinaryOperator>();
    ///
    /// assert!(ids_trinary_operator.contains('\u{2FF2}'));  // IDEOGRAPHIC DESCRIPTION CHARACTER LEFT TO MIDDLE AND RIGHT
    /// assert!(ids_trinary_operator.contains('\u{2FF3}'));  // IDEOGRAPHIC DESCRIPTION CHARACTER ABOVE TO MIDDLE AND BELOW
    /// assert!(!ids_trinary_operator.contains('\u{2FF4}'));
    /// assert!(!ids_trinary_operator.contains('\u{2FF5}'));  // IDEOGRAPHIC DESCRIPTION CHARACTER SURROUND FROM ABOVE
    /// assert!(!ids_trinary_operator.contains('\u{3006}'));  // IDEOGRAPHIC CLOSING MARK
    /// ```
}

make_binary_property! {
    name: "IDS_Unary_Operator";
    short_name: "IDSU";
    ident: IdsUnaryOperator;
    data_marker: crate::provider::PropertyBinaryIdsUnaryOperatorV1;
    singleton: SINGLETON_PROPERTY_BINARY_IDS_UNARY_OPERATOR_V1;
    /// IDS_Unary_Operator Property
}

make_binary_property! {
    name: "Join_Control";
    short_name: "Join_C";
    ident: JoinControl;
    data_marker: crate::provider::PropertyBinaryJoinControlV1;
    singleton: SINGLETON_PROPERTY_BINARY_JOIN_CONTROL_V1;
    /// Format control characters which have specific functions for control of cursive joining
    /// and ligation.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::JoinControl;
    ///
    /// let join_control = CodePointSetData::new::<JoinControl>();
    ///
    /// assert!(join_control.contains('\u{200C}'));  // ZERO WIDTH NON-JOINER
    /// assert!(join_control.contains('\u{200D}'));  // ZERO WIDTH JOINER
    /// assert!(!join_control.contains('\u{200E}'));
    /// ```
}

make_binary_property! {
    name: "Logical_Order_Exception";
    short_name: "LOE";
    ident: LogicalOrderException;
    data_marker: crate::provider::PropertyBinaryLogicalOrderExceptionV1;
    singleton: SINGLETON_PROPERTY_BINARY_LOGICAL_ORDER_EXCEPTION_V1;
    /// A small number of spacing vowel letters occurring in certain Southeast Asian scripts such as Thai and Lao.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::LogicalOrderException;
    ///
    /// let logical_order_exception = CodePointSetData::new::<LogicalOrderException>();
    ///
    /// assert!(logical_order_exception.contains('ແ'));  // U+0EC1 LAO VOWEL SIGN EI
    /// assert!(!logical_order_exception.contains('ະ'));  // U+0EB0 LAO VOWEL SIGN A
    /// ```
}

make_binary_property! {
    name: "Lowercase";
    short_name: "Lower";
    ident: Lowercase;
    data_marker: crate::provider::PropertyBinaryLowercaseV1;
    singleton: SINGLETON_PROPERTY_BINARY_LOWERCASE_V1;
    /// Lowercase characters.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Lowercase;
    ///
    /// let lowercase = CodePointSetData::new::<Lowercase>();
    ///
    /// assert!(lowercase.contains('a'));
    /// assert!(!lowercase.contains('A'));
    /// ```
}

make_binary_property! {
    name: "Math";
    short_name: "Math";
    ident: Math;
    data_marker: crate::provider::PropertyBinaryMathV1;
    singleton: SINGLETON_PROPERTY_BINARY_MATH_V1;
    /// Characters used in mathematical notation.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Math;
    ///
    /// let math = CodePointSetData::new::<Math>();
    ///
    /// assert!(math.contains('='));
    /// assert!(math.contains('+'));
    /// assert!(!math.contains('-'));
    /// assert!(math.contains('−'));  // U+2212 MINUS SIGN
    /// assert!(!math.contains('/'));
    /// assert!(math.contains('∕'));  // U+2215 DIVISION SLASH
    /// ```
}

make_binary_property! {
    name: "Modifier_Combining_Mark";
    short_name: "MCM";
    ident: ModifierCombiningMark;
    data_marker: crate::provider::PropertyBinaryModifierCombiningMarkV1;
    singleton: SINGLETON_PROPERTY_BINARY_MODIFIER_COMBINING_MARK_V1;
    /// Modifier_Combining_Mark Property
}

make_binary_property! {
    name: "Noncharacter_Code_Point";
    short_name: "NChar";
    ident: NoncharacterCodePoint;
    data_marker: crate::provider::PropertyBinaryNoncharacterCodePointV1;
    singleton: SINGLETON_PROPERTY_BINARY_NONCHARACTER_CODE_POINT_V1;
    /// Code points permanently reserved for internal use.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::NoncharacterCodePoint;
    ///
    /// let noncharacter_code_point = CodePointSetData::new::<NoncharacterCodePoint>();
    ///
    /// assert!(noncharacter_code_point.contains('\u{FDD0}'));
    /// assert!(noncharacter_code_point.contains('\u{FFFF}'));
    /// assert!(!noncharacter_code_point.contains('\u{10000}'));
    /// ```
}

make_binary_property! {
    name: "NFC_Inert";
    short_name: "NFC_Inert";
    ident: NfcInert;
    data_marker: crate::provider::PropertyBinaryNfcInertV1;
    singleton: SINGLETON_PROPERTY_BINARY_NFC_INERT_V1;
    /// Characters that are inert under NFC, i.e., they do not interact with adjacent characters.
}

make_binary_property! {
    name: "NFD_Inert";
    short_name: "NFD_Inert";
    ident: NfdInert;
    data_marker: crate::provider::PropertyBinaryNfdInertV1;
    singleton: SINGLETON_PROPERTY_BINARY_NFD_INERT_V1;
    /// Characters that are inert under NFD, i.e., they do not interact with adjacent characters.
}

make_binary_property! {
    name: "NFKC_Inert";
    short_name: "NFKC_Inert";
    ident: NfkcInert;
    data_marker: crate::provider::PropertyBinaryNfkcInertV1;
    singleton: SINGLETON_PROPERTY_BINARY_NFKC_INERT_V1;
    /// Characters that are inert under NFKC, i.e., they do not interact with adjacent characters.
}

make_binary_property! {
    name: "NFKD_Inert";
    short_name: "NFKD_Inert";
    ident: NfkdInert;
    data_marker: crate::provider::PropertyBinaryNfkdInertV1;
    singleton: SINGLETON_PROPERTY_BINARY_NFKD_INERT_V1;
    /// Characters that are inert under NFKD, i.e., they do not interact with adjacent characters.
}

make_binary_property! {
    name: "Pattern_Syntax";
    short_name: "Pat_Syn";
    ident: PatternSyntax;
    data_marker: crate::provider::PropertyBinaryPatternSyntaxV1;
    singleton: SINGLETON_PROPERTY_BINARY_PATTERN_SYNTAX_V1;
    /// Characters used as syntax in patterns (such as regular expressions).
    ///
    /// See [`Unicode
    /// Standard Annex #31`](https://www.unicode.org/reports/tr31/tr31-35.html) for more
    /// details.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::PatternSyntax;
    ///
    /// let pattern_syntax = CodePointSetData::new::<PatternSyntax>();
    ///
    /// assert!(pattern_syntax.contains('{'));
    /// assert!(pattern_syntax.contains('⇒'));  // U+21D2 RIGHTWARDS DOUBLE ARROW
    /// assert!(!pattern_syntax.contains('0'));
    /// ```
}

make_binary_property! {
    name: "Pattern_White_Space";
    short_name: "Pat_WS";
    ident: PatternWhiteSpace;
    data_marker: crate::provider::PropertyBinaryPatternWhiteSpaceV1;
    singleton: SINGLETON_PROPERTY_BINARY_PATTERN_WHITE_SPACE_V1;
    /// Characters used as whitespace in patterns (such as regular expressions).
    ///
    /// See
    /// [`Unicode Standard Annex #31`](https://www.unicode.org/reports/tr31/tr31-35.html) for
    /// more details.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::PatternWhiteSpace;
    ///
    /// let pattern_white_space = CodePointSetData::new::<PatternWhiteSpace>();
    ///
    /// assert!(pattern_white_space.contains(' '));
    /// assert!(pattern_white_space.contains('\u{2029}'));  // PARAGRAPH SEPARATOR
    /// assert!(pattern_white_space.contains('\u{000A}'));  // NEW LINE
    /// assert!(!pattern_white_space.contains('\u{00A0}'));  // NO-BREAK SPACE
    /// ```
}

make_binary_property! {
    name: "Prepended_Concatenation_Mark";
    short_name: "PCM";
    ident: PrependedConcatenationMark;
    data_marker: crate::provider::PropertyBinaryPrependedConcatenationMarkV1;
    singleton: SINGLETON_PROPERTY_BINARY_PREPENDED_CONCATENATION_MARK_V1;
    /// A small class of visible format controls, which precede and then span a sequence of
    /// other characters, usually digits.
}

make_binary_property! {
    name: "Print";
    short_name: "Print";
    ident: Print;
    data_marker: crate::provider::PropertyBinaryPrintV1;
    singleton: SINGLETON_PROPERTY_BINARY_PRINT_V1;
    /// Printable characters (visible characters and whitespace).
    ///
    /// This is defined for POSIX compatibility.
}

make_binary_property! {
    name: "Quotation_Mark";
    short_name: "QMark";
    ident: QuotationMark;
    data_marker: crate::provider::PropertyBinaryQuotationMarkV1;
    singleton: SINGLETON_PROPERTY_BINARY_QUOTATION_MARK_V1;
    /// Punctuation characters that function as quotation marks.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::QuotationMark;
    ///
    /// let quotation_mark = CodePointSetData::new::<QuotationMark>();
    ///
    /// assert!(quotation_mark.contains('\''));
    /// assert!(quotation_mark.contains('„'));  // U+201E DOUBLE LOW-9 QUOTATION MARK
    /// assert!(!quotation_mark.contains('<'));
    /// ```
}

make_binary_property! {
    name: "Radical";
    short_name: "Radical";
    ident: Radical;
    data_marker: crate::provider::PropertyBinaryRadicalV1;
    singleton: SINGLETON_PROPERTY_BINARY_RADICAL_V1;
    /// Characters used in the definition of Ideographic Description Sequences.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Radical;
    ///
    /// let radical = CodePointSetData::new::<Radical>();
    ///
    /// assert!(radical.contains('⺆'));  // U+2E86 CJK RADICAL BOX
    /// assert!(!radical.contains('丹'));  // U+F95E CJK COMPATIBILITY IDEOGRAPH-F95E
    /// ```
}

make_binary_property! {
    name: "Regional_Indicator";
    short_name: "RI";
    ident: RegionalIndicator;
    data_marker: crate::provider::PropertyBinaryRegionalIndicatorV1;
    singleton: SINGLETON_PROPERTY_BINARY_REGIONAL_INDICATOR_V1;
    /// Regional indicator characters, `U+1F1E6..U+1F1FF`.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::RegionalIndicator;
    ///
    /// let regional_indicator = CodePointSetData::new::<RegionalIndicator>();
    ///
    /// assert!(regional_indicator.contains('🇹'));  // U+1F1F9 REGIONAL INDICATOR SYMBOL LETTER T
    /// assert!(!regional_indicator.contains('Ⓣ'));  // U+24C9 CIRCLED LATIN CAPITAL LETTER T
    /// assert!(!regional_indicator.contains('T'));
    /// ```
}

make_binary_property! {
    name: "Soft_Dotted";
    short_name: "SD";
    ident: SoftDotted;
    data_marker: crate::provider::PropertyBinarySoftDottedV1;
    singleton: SINGLETON_PROPERTY_BINARY_SOFT_DOTTED_V1;
    /// Characters with a "soft dot", like i or j.
    ///
    /// An accent placed on these characters causes
    /// the dot to disappear.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::SoftDotted;
    ///
    /// let soft_dotted = CodePointSetData::new::<SoftDotted>();
    ///
    /// assert!(soft_dotted.contains('і'));  //U+0456 CYRILLIC SMALL LETTER BYELORUSSIAN-UKRAINIAN I
    /// assert!(!soft_dotted.contains('ı'));  // U+0131 LATIN SMALL LETTER DOTLESS I
    /// ```
}

make_binary_property! {
    name: "Segment_Starter";
    short_name: "Segment_Starter";
    ident: SegmentStarter;
    data_marker: crate::provider::PropertyBinarySegmentStarterV1;
    singleton: SINGLETON_PROPERTY_BINARY_SEGMENT_STARTER_V1;
    /// Characters that are starters in terms of Unicode normalization and combining character
    /// sequences.
}

make_binary_property! {
    name: "Case_Sensitive";
    short_name: "Case_Sensitive";
    ident: CaseSensitive;
    data_marker: crate::provider::PropertyBinaryCaseSensitiveV1;
    singleton: SINGLETON_PROPERTY_BINARY_CASE_SENSITIVE_V1;
    /// Characters that are either the source of a case mapping or in the target of a case
    /// mapping.
}

make_binary_property! {
    name: "Sentence_Terminal";
    short_name: "STerm";
    ident: SentenceTerminal;
    data_marker: crate::provider::PropertyBinarySentenceTerminalV1;
    singleton: SINGLETON_PROPERTY_BINARY_SENTENCE_TERMINAL_V1;
    /// Punctuation characters that generally mark the end of sentences.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::SentenceTerminal;
    ///
    /// let sentence_terminal = CodePointSetData::new::<SentenceTerminal>();
    ///
    /// assert!(sentence_terminal.contains('.'));
    /// assert!(sentence_terminal.contains('?'));
    /// assert!(sentence_terminal.contains('᪨'));  // U+1AA8 TAI THAM SIGN KAAN
    /// assert!(!sentence_terminal.contains(','));
    /// assert!(!sentence_terminal.contains('¿'));  // U+00BF INVERTED QUESTION MARK
    /// ```
}

make_binary_property! {
    name: "Terminal_Punctuation";
    short_name: "Term";
    ident: TerminalPunctuation;
    data_marker: crate::provider::PropertyBinaryTerminalPunctuationV1;
    singleton: SINGLETON_PROPERTY_BINARY_TERMINAL_PUNCTUATION_V1;
    /// Punctuation characters that generally mark the end of textual units.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::TerminalPunctuation;
    ///
    /// let terminal_punctuation = CodePointSetData::new::<TerminalPunctuation>();
    ///
    /// assert!(terminal_punctuation.contains('.'));
    /// assert!(terminal_punctuation.contains('?'));
    /// assert!(terminal_punctuation.contains('᪨'));  // U+1AA8 TAI THAM SIGN KAAN
    /// assert!(terminal_punctuation.contains(','));
    /// assert!(!terminal_punctuation.contains('¿'));  // U+00BF INVERTED QUESTION MARK
    /// ```
}

make_binary_property! {
    name: "Unified_Ideograph";
    short_name: "UIdeo";
    ident: UnifiedIdeograph;
    data_marker: crate::provider::PropertyBinaryUnifiedIdeographV1;
    singleton: SINGLETON_PROPERTY_BINARY_UNIFIED_IDEOGRAPH_V1;
    /// A property which specifies the exact set of Unified CJK Ideographs in the standard.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::UnifiedIdeograph;
    ///
    /// let unified_ideograph = CodePointSetData::new::<UnifiedIdeograph>();
    ///
    /// assert!(unified_ideograph.contains('川'));  // U+5DDD CJK UNIFIED IDEOGRAPH-5DDD
    /// assert!(unified_ideograph.contains('木'));  // U+6728 CJK UNIFIED IDEOGRAPH-6728
    /// assert!(!unified_ideograph.contains('𛅸'));  // U+1B178 NUSHU CHARACTER-1B178
    /// ```
}

make_binary_property! {
    name: "Uppercase";
    short_name: "Upper";
    ident: Uppercase;
    data_marker: crate::provider::PropertyBinaryUppercaseV1;
    singleton: SINGLETON_PROPERTY_BINARY_UPPERCASE_V1;
    /// Uppercase characters.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Uppercase;
    ///
    /// let uppercase = CodePointSetData::new::<Uppercase>();
    ///
    /// assert!(uppercase.contains('U'));
    /// assert!(!uppercase.contains('u'));
    /// ```
}

make_binary_property! {
    name: "Variation_Selector";
    short_name: "VS";
    ident: VariationSelector;
    data_marker: crate::provider::PropertyBinaryVariationSelectorV1;
    singleton: SINGLETON_PROPERTY_BINARY_VARIATION_SELECTOR_V1;
    /// Characters that are Variation Selectors.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::VariationSelector;
    ///
    /// let variation_selector = CodePointSetData::new::<VariationSelector>();
    ///
    /// assert!(variation_selector.contains('\u{180D}'));  // MONGOLIAN FREE VARIATION SELECTOR THREE
    /// assert!(!variation_selector.contains('\u{303E}'));  // IDEOGRAPHIC VARIATION INDICATOR
    /// assert!(variation_selector.contains('\u{FE0F}'));  // VARIATION SELECTOR-16
    /// assert!(!variation_selector.contains('\u{FE10}'));  // PRESENTATION FORM FOR VERTICAL COMMA
    /// assert!(variation_selector.contains('\u{E01EF}'));  // VARIATION SELECTOR-256
    /// ```
}

make_binary_property! {
    name: "White_Space";
    short_name: "space";
    ident: WhiteSpace;
    data_marker: crate::provider::PropertyBinaryWhiteSpaceV1;
    singleton: SINGLETON_PROPERTY_BINARY_WHITE_SPACE_V1;
    /// Spaces, separator characters and other control characters which should be treated by
    /// programming languages as "white space" for the purpose of parsing elements.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::WhiteSpace;
    ///
    /// let white_space = CodePointSetData::new::<WhiteSpace>();
    ///
    /// assert!(white_space.contains(' '));
    /// assert!(white_space.contains('\u{000A}'));  // NEW LINE
    /// assert!(white_space.contains('\u{00A0}'));  // NO-BREAK SPACE
    /// assert!(!white_space.contains('\u{200B}'));  // ZERO WIDTH SPACE
    /// ```
}

make_binary_property! {
    name: "Xdigit";
    short_name: "Xdigit";
    ident: Xdigit;
    data_marker: crate::provider::PropertyBinaryXdigitV1;
    singleton: SINGLETON_PROPERTY_BINARY_XDIGIT_V1;
    /// Hexadecimal digits
    ///
    /// This is defined for POSIX compatibility.
}

make_binary_property! {
    name: "XID_Continue";
    short_name: "XIDC";
    ident: XidContinue;
    data_marker: crate::provider::PropertyBinaryXidContinueV1;
    singleton: SINGLETON_PROPERTY_BINARY_XID_CONTINUE_V1;
    /// Characters that can come after the first character in an identifier.
    ///
    /// See [`Unicode Standard Annex
    /// #31`](https://www.unicode.org/reports/tr31/tr31-35.html) for more details.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::XidContinue;
    ///
    /// let xid_continue = CodePointSetData::new::<XidContinue>();
    ///
    /// assert!(xid_continue.contains('x'));
    /// assert!(xid_continue.contains('1'));
    /// assert!(xid_continue.contains('_'));
    /// assert!(xid_continue.contains('ߝ'));  // U+07DD NKO LETTER FA
    /// assert!(!xid_continue.contains('ⓧ'));  // U+24E7 CIRCLED LATIN SMALL LETTER X
    /// assert!(!xid_continue.contains('\u{FC5E}'));  // ARABIC LIGATURE SHADDA WITH DAMMATAN ISOLATED FORM
    /// ```
}

make_binary_property! {
    name: "XID_Start";
    short_name: "XIDS";
    ident: XidStart;
    data_marker: crate::provider::PropertyBinaryXidStartV1;
    singleton: SINGLETON_PROPERTY_BINARY_XID_START_V1;
    /// Characters that can begin an identifier.
    ///
    /// See [`Unicode
    /// Standard Annex #31`](https://www.unicode.org/reports/tr31/tr31-35.html) for more
    /// details.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::XidStart;
    ///
    /// let xid_start = CodePointSetData::new::<XidStart>();
    ///
    /// assert!(xid_start.contains('x'));
    /// assert!(!xid_start.contains('1'));
    /// assert!(!xid_start.contains('_'));
    /// assert!(xid_start.contains('ߝ'));  // U+07DD NKO LETTER FA
    /// assert!(!xid_start.contains('ⓧ'));  // U+24E7 CIRCLED LATIN SMALL LETTER X
    /// assert!(!xid_start.contains('\u{FC5E}'));  // ARABIC LIGATURE SHADDA WITH DAMMATAN ISOLATED FORM
    /// ```
}

pub use crate::emoji::EmojiSet;

macro_rules! make_emoji_set {
    (
        ident: $ident:ident;
        data_marker: $data_marker:ty;
        singleton: $singleton:ident;
        $(#[$doc:meta])+
    ) => {
        $(#[$doc])+
        #[derive(Debug)]
        #[non_exhaustive]
        pub struct $ident;

        impl crate::private::Sealed for $ident {}

        impl EmojiSet for $ident {
            type DataMarker = $data_marker;
            #[cfg(feature = "compiled_data")]
            const SINGLETON: &'static crate::provider::PropertyUnicodeSet<'static> =
                &crate::provider::Baked::$singleton;
        }
    }
}

make_emoji_set! {
    ident: BasicEmoji;
    data_marker: crate::provider::PropertyBinaryBasicEmojiV1;
    singleton: SINGLETON_PROPERTY_BINARY_BASIC_EMOJI_V1;
    /// Characters and character sequences intended for general-purpose, independent, direct input.
    ///
    /// See [`Unicode Technical Standard #51`](https://unicode.org/reports/tr51/) for more
    /// details.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::EmojiSetData;
    /// use icu::properties::props::BasicEmoji;
    ///
    /// let basic_emoji = EmojiSetData::new::<BasicEmoji>();
    ///
    /// assert!(!basic_emoji.contains('\u{0020}'));
    /// assert!(!basic_emoji.contains('\n'));
    /// assert!(basic_emoji.contains('🦃')); // U+1F983 TURKEY
    /// assert!(basic_emoji.contains_str("\u{1F983}"));
    /// assert!(basic_emoji.contains_str("\u{1F6E4}\u{FE0F}")); // railway track
    /// assert!(!basic_emoji.contains_str("\u{0033}\u{FE0F}\u{20E3}"));  // Emoji_Keycap_Sequence, keycap 3
    /// ```
}

#[cfg(test)]
mod test_enumerated_property_completeness {
    use super::*;
    use std::collections::BTreeMap;

    fn check_enum<'a, T: NamedEnumeratedProperty>(
        lookup: &crate::provider::names::PropertyValueNameToEnumMap<'static>,
        consts: impl IntoIterator<Item = &'a T>,
    ) where
        u16: From<T>,
    {
        let mut data: BTreeMap<_, _> = lookup
            .map
            .iter()
            .map(|(name, value)| (value, (name, "Data")))
            .collect();

        let names = crate::PropertyNamesLong::<T>::new();
        let consts = consts.into_iter().map(|value| {
            (
                u16::from(*value) as usize,
                (
                    names.get(*value).unwrap_or("<unknown>").to_string(),
                    "Consts",
                ),
            )
        });

        let mut diff = Vec::new();
        for t @ (value, _) in consts {
            if data.remove(&value).is_none() {
                diff.push(t);
            }
        }
        diff.extend(data);

        let mut fmt_diff = String::new();
        for (value, (name, source)) in diff {
            fmt_diff.push_str(&format!("{source}:\t{name} = {value:?}\n"));
        }

        assert!(
            fmt_diff.is_empty(),
            "Values defined in data do not match values defined in consts. Difference:\n{fmt_diff}"
        );
    }

    #[test]
    fn test_ea() {
        check_enum(
            crate::provider::Baked::SINGLETON_PROPERTY_NAME_PARSE_EAST_ASIAN_WIDTH_V1,
            EastAsianWidth::ALL_VALUES,
        );
    }

    #[test]
    fn test_ccc() {
        check_enum(
            crate::provider::Baked::SINGLETON_PROPERTY_NAME_PARSE_CANONICAL_COMBINING_CLASS_V1,
            CanonicalCombiningClass::ALL_VALUES,
        );
    }

    #[test]
    fn test_jt() {
        check_enum(
            crate::provider::Baked::SINGLETON_PROPERTY_NAME_PARSE_JOINING_TYPE_V1,
            JoiningType::ALL_VALUES,
        );
    }

    #[test]
    fn test_insc() {
        check_enum(
            crate::provider::Baked::SINGLETON_PROPERTY_NAME_PARSE_INDIC_SYLLABIC_CATEGORY_V1,
            IndicSyllabicCategory::ALL_VALUES,
        );
    }

    #[test]
    fn test_sb() {
        check_enum(
            crate::provider::Baked::SINGLETON_PROPERTY_NAME_PARSE_SENTENCE_BREAK_V1,
            SentenceBreak::ALL_VALUES,
        );
    }

    #[test]
    fn test_wb() {
        check_enum(
            crate::provider::Baked::SINGLETON_PROPERTY_NAME_PARSE_WORD_BREAK_V1,
            WordBreak::ALL_VALUES,
        );
    }

    #[test]
    fn test_bc() {
        check_enum(
            crate::provider::Baked::SINGLETON_PROPERTY_NAME_PARSE_BIDI_CLASS_V1,
            BidiClass::ALL_VALUES,
        );
    }

    #[test]
    fn test_hst() {
        check_enum(
            crate::provider::Baked::SINGLETON_PROPERTY_NAME_PARSE_HANGUL_SYLLABLE_TYPE_V1,
            HangulSyllableType::ALL_VALUES,
        );
    }

    #[test]
    fn test_vo() {
        check_enum(
            crate::provider::Baked::SINGLETON_PROPERTY_NAME_PARSE_VERTICAL_ORIENTATION_V1,
            VerticalOrientation::ALL_VALUES,
        );
    }
}
