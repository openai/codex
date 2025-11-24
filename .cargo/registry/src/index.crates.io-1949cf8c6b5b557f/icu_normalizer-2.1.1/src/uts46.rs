// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Bundles the part of UTS 46 that makes sense to implement as a
//! normalization.
//!
//! This is meant to be used as a building block of an UTS 46
//! implementation, such as the `idna` crate.

use crate::ComposingNormalizer;
use crate::ComposingNormalizerBorrowed;
use crate::NormalizerNfcV1;
use crate::NormalizerNfdTablesV1;
use crate::NormalizerNfkdTablesV1;
use crate::NormalizerUts46DataV1;
use icu_provider::DataError;
use icu_provider::DataProvider;

// Implementation note: Despite merely wrapping a `ComposingNormalizer`,
// having a `Uts46Mapper` serves two purposes:
//
// 1. Denying public access to parts of the `ComposingNormalizer` API
//    that don't work when the data contains markers for ignorables.
// 2. Providing a place where additional iterator pre-processing or
//    post-processing can take place if needed in the future. (When
//    writing this, it looked like such processing was needed but
//    now isn't needed after all.)

/// A borrowed version of a mapper that knows how to performs the
/// subsets of UTS 46 processing documented on the methods.
#[derive(Debug)]
pub struct Uts46MapperBorrowed<'a> {
    normalizer: ComposingNormalizerBorrowed<'a>,
}

#[cfg(feature = "compiled_data")]
impl Default for Uts46MapperBorrowed<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl Uts46MapperBorrowed<'static> {
    /// Cheaply converts a [`Uts46MapperBorrowed<'static>`] into a [`Uts46Mapper`].
    ///
    /// Note: Due to branching and indirection, using [`Uts46Mapper`] might inhibit some
    /// compile-time optimizations that are possible with [`Uts46MapperBorrowed`].
    pub const fn static_to_owned(self) -> Uts46Mapper {
        Uts46Mapper {
            normalizer: self.normalizer.static_to_owned(),
        }
    }

    /// Construct with compiled data.
    #[cfg(feature = "compiled_data")]
    pub const fn new() -> Self {
        Uts46MapperBorrowed {
            normalizer: ComposingNormalizerBorrowed::new_uts46(),
        }
    }
}

impl Uts46MapperBorrowed<'_> {
    /// Returns an iterator adaptor that turns an `Iterator` over `char`
    /// into an iterator yielding a `char` sequence that gets the following
    /// operations from the "Map" and "Normalize" steps of the "Processing"
    /// section of UTS 46 lazily applied to it:
    ///
    /// 1. The _ignored_ characters are ignored.
    /// 2. The _mapped_ characters are mapped.
    /// 3. The _disallowed_ characters are replaced with U+FFFD,
    ///    which itself is a disallowed character.
    /// 4. The _deviation_ characters are treated as _mapped_ or _valid_
    ///    as appropriate.
    /// 5. The _disallowed_STD3_valid_ characters are treated as allowed.
    /// 6. The _disallowed_STD3_mapped_ characters are treated as
    ///    _mapped_.
    /// 7. The result is normalized to NFC.
    ///
    /// Notably:
    ///
    /// * The STD3 or WHATWG ASCII deny list should be implemented as a
    ///   post-processing step.
    /// * Transitional processing is not performed. Transitional mapping
    ///   would be a pre-processing step, but transitional processing is
    ///   deprecated, and none of Firefox, Safari, or Chrome use it.
    pub fn map_normalize<'delegate, I: Iterator<Item = char> + 'delegate>(
        &'delegate self,
        iter: I,
    ) -> impl Iterator<Item = char> + 'delegate {
        self.normalizer
            .normalize_iter_private(iter, crate::IgnorableBehavior::Ignored)
    }

    /// Returns an iterator adaptor that turns an `Iterator` over `char`
    /// into an iterator yielding a `char` sequence that gets the following
    /// operations from the NFC check and statucs steps of the "Validity
    /// Criteria" section of UTS 46 lazily applied to it:
    ///
    /// 1. The _ignored_ characters are treated as _disallowed_.
    /// 2. The _mapped_ characters are mapped.
    /// 3. The _disallowed_ characters are replaced with U+FFFD,
    ///    which itself is a disallowed character.
    /// 4. The _deviation_ characters are treated as _mapped_ or _valid_
    ///    as appropriate.
    /// 5. The _disallowed_STD3_valid_ characters are treated as allowed.
    /// 6. The _disallowed_STD3_mapped_ characters are treated as
    ///    _mapped_.
    /// 7. The result is normalized to NFC.
    ///
    /// Notably:
    ///
    /// * The STD3 or WHATWG ASCII deny list should be implemented as a
    ///   post-processing step.
    /// * Transitional processing is not performed. Transitional mapping
    ///   would be a pre-processing step, but transitional processing is
    ///   deprecated, and none of Firefox, Safari, or Chrome use it.
    /// * The output needs to be compared with input to see if anything
    ///   changed. This check catches failures to adhere to the normalization
    ///   and status requirements. In particular, this comparison results
    ///   in _mapped_ characters resulting in error like "Validity Criteria"
    ///   requires.
    pub fn normalize_validate<'delegate, I: Iterator<Item = char> + 'delegate>(
        &'delegate self,
        iter: I,
    ) -> impl Iterator<Item = char> + 'delegate {
        self.normalizer
            .normalize_iter_private(iter, crate::IgnorableBehavior::ReplacementCharacter)
    }
}

/// A mapper that knows how to performs the subsets of UTS 46 processing
/// documented on the methods.
#[derive(Debug)]
pub struct Uts46Mapper {
    normalizer: ComposingNormalizer,
}

#[cfg(feature = "compiled_data")]
impl Default for Uts46Mapper {
    fn default() -> Self {
        Self::new().static_to_owned()
    }
}

impl Uts46Mapper {
    /// Constructs a borrowed version of this type for more efficient querying.
    pub fn as_borrowed(&self) -> Uts46MapperBorrowed<'_> {
        Uts46MapperBorrowed {
            normalizer: self.normalizer.as_borrowed(),
        }
    }

    /// Construct with compiled data.
    #[cfg(feature = "compiled_data")]
    #[expect(clippy::new_ret_no_self)]
    pub const fn new() -> Uts46MapperBorrowed<'static> {
        Uts46MapperBorrowed::new()
    }

    /// Construct with provider.
    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new)]
    pub fn try_new<D>(provider: &D) -> Result<Self, DataError>
    where
        D: DataProvider<NormalizerUts46DataV1>
            + DataProvider<NormalizerNfdTablesV1>
            + DataProvider<NormalizerNfkdTablesV1>
            // UTS 46 tables merged into NormalizerNfkdTablesV1
            + DataProvider<NormalizerNfcV1>
            + ?Sized,
    {
        let normalizer = ComposingNormalizer::try_new_uts46_unstable(provider)?;

        Ok(Uts46Mapper { normalizer })
    }
}
