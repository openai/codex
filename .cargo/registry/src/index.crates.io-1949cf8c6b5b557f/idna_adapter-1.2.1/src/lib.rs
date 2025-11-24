// Copyright The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This crate abstracts over a Unicode back end for the [`idna`][1]
//! crate.
//!
//! To work around the lack of [`global-features`][2] in Cargo, this
//! crate allows the top level `Cargo.lock` to choose an alternative
//! Unicode back end for the `idna` crate by pinning a version of this
//! crate.
//!
//! See the [README of the latest version][3] for more details.
//!
//! [1]: https://docs.rs/crate/idna/latest
//! [2]: https://internals.rust-lang.org/t/pre-rfc-mutually-excusive-global-features/19618
//! [3]: https://docs.rs/crate/idna_adapter/latest

#![no_std]

use icu_normalizer::properties::CanonicalCombiningClassMapBorrowed;
use icu_normalizer::uts46::Uts46MapperBorrowed;
use icu_properties::props::GeneralCategory;
use icu_properties::CodePointMapDataBorrowed;

/// Turns a joining type into a mask for comparing with multiple type at once.
const fn joining_type_to_mask(jt: icu_properties::props::JoiningType) -> u32 {
    1u32 << jt.to_icu4c_value()
}

/// Mask for checking for both left and dual joining.
pub const LEFT_OR_DUAL_JOINING_MASK: JoiningTypeMask = JoiningTypeMask(
    joining_type_to_mask(icu_properties::props::JoiningType::LeftJoining)
        | joining_type_to_mask(icu_properties::props::JoiningType::DualJoining),
);

/// Mask for checking for both left and dual joining.
pub const RIGHT_OR_DUAL_JOINING_MASK: JoiningTypeMask = JoiningTypeMask(
    joining_type_to_mask(icu_properties::props::JoiningType::RightJoining)
        | joining_type_to_mask(icu_properties::props::JoiningType::DualJoining),
);

/// Turns a bidi class into a mask for comparing with multiple classes at once.
const fn bidi_class_to_mask(bc: icu_properties::props::BidiClass) -> u32 {
    1u32 << bc.to_icu4c_value()
}

/// Mask for checking if the domain is a bidi domain.
pub const RTL_MASK: BidiClassMask = BidiClassMask(
    bidi_class_to_mask(icu_properties::props::BidiClass::RightToLeft)
        | bidi_class_to_mask(icu_properties::props::BidiClass::ArabicLetter)
        | bidi_class_to_mask(icu_properties::props::BidiClass::ArabicNumber),
);

/// Mask for allowable bidi classes in the first character of a label
/// (either LTR or RTL) in a bidi domain.
pub const FIRST_BC_MASK: BidiClassMask = BidiClassMask(
    bidi_class_to_mask(icu_properties::props::BidiClass::LeftToRight)
        | bidi_class_to_mask(icu_properties::props::BidiClass::RightToLeft)
        | bidi_class_to_mask(icu_properties::props::BidiClass::ArabicLetter),
);

// Mask for allowable bidi classes of the last (non-Non-Spacing Mark)
// character in an LTR label in a bidi domain.
pub const LAST_LTR_MASK: BidiClassMask = BidiClassMask(
    bidi_class_to_mask(icu_properties::props::BidiClass::LeftToRight)
        | bidi_class_to_mask(icu_properties::props::BidiClass::EuropeanNumber),
);

// Mask for allowable bidi classes of the last (non-Non-Spacing Mark)
// character in an RTL label in a bidi domain.
pub const LAST_RTL_MASK: BidiClassMask = BidiClassMask(
    bidi_class_to_mask(icu_properties::props::BidiClass::RightToLeft)
        | bidi_class_to_mask(icu_properties::props::BidiClass::ArabicLetter)
        | bidi_class_to_mask(icu_properties::props::BidiClass::EuropeanNumber)
        | bidi_class_to_mask(icu_properties::props::BidiClass::ArabicNumber),
);

// Mask for allowable bidi classes of the middle characters in an LTR label in a bidi domain.
pub const MIDDLE_LTR_MASK: BidiClassMask = BidiClassMask(
    bidi_class_to_mask(icu_properties::props::BidiClass::LeftToRight)
        | bidi_class_to_mask(icu_properties::props::BidiClass::EuropeanNumber)
        | bidi_class_to_mask(icu_properties::props::BidiClass::EuropeanSeparator)
        | bidi_class_to_mask(icu_properties::props::BidiClass::CommonSeparator)
        | bidi_class_to_mask(icu_properties::props::BidiClass::EuropeanTerminator)
        | bidi_class_to_mask(icu_properties::props::BidiClass::OtherNeutral)
        | bidi_class_to_mask(icu_properties::props::BidiClass::BoundaryNeutral)
        | bidi_class_to_mask(icu_properties::props::BidiClass::NonspacingMark),
);

// Mask for allowable bidi classes of the middle characters in an RTL label in a bidi domain.
pub const MIDDLE_RTL_MASK: BidiClassMask = BidiClassMask(
    bidi_class_to_mask(icu_properties::props::BidiClass::RightToLeft)
        | bidi_class_to_mask(icu_properties::props::BidiClass::ArabicLetter)
        | bidi_class_to_mask(icu_properties::props::BidiClass::ArabicNumber)
        | bidi_class_to_mask(icu_properties::props::BidiClass::EuropeanNumber)
        | bidi_class_to_mask(icu_properties::props::BidiClass::EuropeanSeparator)
        | bidi_class_to_mask(icu_properties::props::BidiClass::CommonSeparator)
        | bidi_class_to_mask(icu_properties::props::BidiClass::EuropeanTerminator)
        | bidi_class_to_mask(icu_properties::props::BidiClass::OtherNeutral)
        | bidi_class_to_mask(icu_properties::props::BidiClass::BoundaryNeutral)
        | bidi_class_to_mask(icu_properties::props::BidiClass::NonspacingMark),
);

/// Turns a genecal category into a mask for comparing with multiple categories at once.
const fn general_category_to_mask(gc: GeneralCategory) -> u32 {
    1 << (gc as u32)
}

/// Mask for the disallowed general categories of the first character in a label.
const MARK_MASK: u32 = general_category_to_mask(GeneralCategory::NonspacingMark)
    | general_category_to_mask(GeneralCategory::SpacingMark)
    | general_category_to_mask(GeneralCategory::EnclosingMark);

/// Value for the Joining_Type Unicode property.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct JoiningType(icu_properties::props::JoiningType);

impl JoiningType {
    /// Returns the corresponding `JoiningTypeMask`.
    #[inline(always)]
    pub fn to_mask(self) -> JoiningTypeMask {
        JoiningTypeMask(joining_type_to_mask(self.0))
    }

    // `true` iff this value is the Transparent value.
    #[inline(always)]
    pub fn is_transparent(self) -> bool {
        self.0 == icu_properties::props::JoiningType::Transparent
    }
}

/// A mask representing potentially multiple `JoiningType`
/// values.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct JoiningTypeMask(u32);

impl JoiningTypeMask {
    /// `true` iff both masks have at `JoiningType` in common.
    #[inline(always)]
    pub fn intersects(self, other: JoiningTypeMask) -> bool {
        self.0 & other.0 != 0
    }
}

/// Value for the Bidi_Class Unicode property.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct BidiClass(icu_properties::props::BidiClass);

impl BidiClass {
    /// Returns the corresponding `BidiClassMask`.
    #[inline(always)]
    pub fn to_mask(self) -> BidiClassMask {
        BidiClassMask(bidi_class_to_mask(self.0))
    }

    /// `true` iff this value is Left_To_Right
    #[inline(always)]
    pub fn is_ltr(self) -> bool {
        self.0 == icu_properties::props::BidiClass::LeftToRight
    }

    /// `true` iff this value is Nonspacing_Mark
    #[inline(always)]
    pub fn is_nonspacing_mark(self) -> bool {
        self.0 == icu_properties::props::BidiClass::NonspacingMark
    }

    /// `true` iff this value is European_Number
    #[inline(always)]
    pub fn is_european_number(self) -> bool {
        self.0 == icu_properties::props::BidiClass::EuropeanNumber
    }

    /// `true` iff this value is Arabic_Number
    #[inline(always)]
    pub fn is_arabic_number(self) -> bool {
        self.0 == icu_properties::props::BidiClass::ArabicNumber
    }
}

/// A mask representing potentially multiple `BidiClass`
/// values.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct BidiClassMask(u32);

impl BidiClassMask {
    /// `true` iff both masks have at `BidiClass` in common.
    #[inline(always)]
    pub fn intersects(self, other: BidiClassMask) -> bool {
        self.0 & other.0 != 0
    }
}

/// An adapter between a Unicode back end an the `idna` crate.
pub struct Adapter {
    mapper: Uts46MapperBorrowed<'static>,
    canonical_combining_class: CanonicalCombiningClassMapBorrowed<'static>,
    general_category: CodePointMapDataBorrowed<'static, GeneralCategory>,
    bidi_class: CodePointMapDataBorrowed<'static, icu_properties::props::BidiClass>,
    joining_type: CodePointMapDataBorrowed<'static, icu_properties::props::JoiningType>,
}

#[cfg(feature = "compiled_data")]
impl Default for Adapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter {
    /// Constructor using data compiled into the binary.
    #[cfg(feature = "compiled_data")]
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            mapper: Uts46MapperBorrowed::new(),
            canonical_combining_class: CanonicalCombiningClassMapBorrowed::new(),
            general_category: icu_properties::CodePointMapData::<GeneralCategory>::new(),
            bidi_class: icu_properties::CodePointMapData::<icu_properties::props::BidiClass>::new(),
            joining_type:
                icu_properties::CodePointMapData::<icu_properties::props::JoiningType>::new(),
        }
    }

    /// `true` iff the Canonical_Combining_Class of `c` is Virama.
    #[inline(always)]
    pub fn is_virama(&self, c: char) -> bool {
        self.canonical_combining_class.get_u8(c) == 9
    }

    /// `true` iff the General_Category of `c` is Mark, i.e. any of Nonspacing_Mark,
    /// Spacing_Mark, or Enclosing_Mark.
    #[inline(always)]
    pub fn is_mark(&self, c: char) -> bool {
        (general_category_to_mask(self.general_category.get(c)) & MARK_MASK) != 0
    }

    /// Returns the Bidi_Class of `c`.
    #[inline(always)]
    pub fn bidi_class(&self, c: char) -> BidiClass {
        BidiClass(self.bidi_class.get(c))
    }

    /// Returns the Joining_Type of `c`.
    #[inline(always)]
    pub fn joining_type(&self, c: char) -> JoiningType {
        JoiningType(self.joining_type.get(c))
    }

    /// See the [method of the same name in `icu_normalizer`][1] for the
    /// exact semantics.
    ///
    /// [1]: https://docs.rs/icu_normalizer/latest/icu_normalizer/uts46/struct.Uts46Mapper.html#method.map_normalize
    #[inline(always)]
    pub fn map_normalize<'delegate, I: Iterator<Item = char> + 'delegate>(
        &'delegate self,
        iter: I,
    ) -> impl Iterator<Item = char> + 'delegate {
        self.mapper.map_normalize(iter)
    }

    /// See the [method of the same name in `icu_normalizer`][1] for the
    /// exact semantics.
    ///
    /// [1]: https://docs.rs/icu_normalizer/latest/icu_normalizer/uts46/struct.Uts46Mapper.html#method.normalize_validate
    #[inline(always)]
    pub fn normalize_validate<'delegate, I: Iterator<Item = char> + 'delegate>(
        &'delegate self,
        iter: I,
    ) -> impl Iterator<Item = char> + 'delegate {
        self.mapper.normalize_validate(iter)
    }
}
