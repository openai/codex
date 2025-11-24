// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Access to the Unicode properties or property-based operations that
//! are required for NFC and NFD.
//!
//! Applications should generally use the full normalizers that are
//! provided at the top level of this crate. However, the APIs in this
//! module are provided for callers such as HarfBuzz that specifically
//! want access to the raw canonical composition operation e.g. for use in a
//! glyph-availability-guided custom normalizer.

use crate::char_from_u16;
use crate::char_from_u32;
use crate::in_inclusive_range;
use crate::provider::CanonicalCompositions;
use crate::provider::DecompositionData;
use crate::provider::DecompositionTables;
use crate::provider::NonRecursiveDecompositionSupplement;
use crate::provider::NormalizerNfcV1;
use crate::provider::NormalizerNfdDataV1;
use crate::provider::NormalizerNfdSupplementV1;
use crate::provider::NormalizerNfdTablesV1;
use crate::trie_value_has_ccc;
use crate::CanonicalCombiningClass;
use crate::BACKWARD_COMBINING_MARKER;
use crate::FDFA_MARKER;
use crate::HANGUL_L_BASE;
use crate::HANGUL_N_COUNT;
use crate::HANGUL_S_BASE;
use crate::HANGUL_S_COUNT;
use crate::HANGUL_T_BASE;
use crate::HANGUL_T_COUNT;
use crate::HANGUL_V_BASE;
use crate::HIGH_ZEROS_MASK;
use crate::LOW_ZEROS_MASK;
use crate::NON_ROUND_TRIP_MARKER;
use icu_provider::prelude::*;

/// Borrowed version of the raw canonical composition operation.
///
/// Callers should generally use `ComposingNormalizer` instead of this API.
/// However, this API is provided for callers such as HarfBuzz that specifically
/// want access to the raw canonical composition operation e.g. for use in a
/// glyph-availability-guided custom normalizer.
#[derive(Debug, Copy, Clone)]
pub struct CanonicalCompositionBorrowed<'a> {
    canonical_compositions: &'a CanonicalCompositions<'a>,
}

#[cfg(feature = "compiled_data")]
impl Default for CanonicalCompositionBorrowed<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl CanonicalCompositionBorrowed<'static> {
    /// Cheaply converts a [`CanonicalCompositionBorrowed<'static>`] into a [`CanonicalComposition`].
    ///
    /// Note: Due to branching and indirection, using [`CanonicalComposition`] might inhibit some
    /// compile-time optimizations that are possible with [`CanonicalCompositionBorrowed`].
    pub const fn static_to_owned(self) -> CanonicalComposition {
        CanonicalComposition {
            canonical_compositions: DataPayload::from_static_ref(self.canonical_compositions),
        }
    }

    /// Constructs a new `CanonicalComposition` using compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub const fn new() -> Self {
        Self {
            canonical_compositions: crate::provider::Baked::SINGLETON_NORMALIZER_NFC_V1,
        }
    }
}

impl CanonicalCompositionBorrowed<'_> {
    /// Performs canonical composition (including Hangul) on a pair of
    /// characters or returns `None` if these characters don't compose.
    /// Composition exclusions are taken into account.
    ///
    /// # Examples
    ///
    /// ```
    /// let comp = icu::normalizer::properties::CanonicalCompositionBorrowed::new();
    ///
    /// assert_eq!(comp.compose('a', 'b'), None); // Just two non-composing starters
    /// assert_eq!(comp.compose('a', '\u{0308}'), Some('ä'));
    /// assert_eq!(comp.compose('ẹ', '\u{0302}'), Some('ệ'));
    /// assert_eq!(comp.compose('𝅗', '𝅥'), None); // Composition exclusion
    /// assert_eq!(comp.compose('ে', 'া'), Some('ো')); // Second is starter
    /// assert_eq!(comp.compose('ᄀ', 'ᅡ'), Some('가')); // Hangul LV
    /// assert_eq!(comp.compose('가', 'ᆨ'), Some('각')); // Hangul LVT
    /// ```
    #[inline(always)]
    pub fn compose(self, starter: char, second: char) -> Option<char> {
        crate::compose(
            self.canonical_compositions.canonical_compositions.iter(),
            starter,
            second,
        )
    }
}

/// The raw canonical composition operation.
///
/// Callers should generally use `ComposingNormalizer` instead of this API.
/// However, this API is provided for callers such as HarfBuzz that specifically
/// want access to the raw canonical composition operation e.g. for use in a
/// glyph-availability-guided custom normalizer.
#[derive(Debug)]
pub struct CanonicalComposition {
    canonical_compositions: DataPayload<NormalizerNfcV1>,
}

#[cfg(feature = "compiled_data")]
impl Default for CanonicalComposition {
    fn default() -> Self {
        Self::new().static_to_owned()
    }
}

impl CanonicalComposition {
    /// Constructs a borrowed version of this type for more efficient querying.
    pub fn as_borrowed(&self) -> CanonicalCompositionBorrowed<'_> {
        CanonicalCompositionBorrowed {
            canonical_compositions: self.canonical_compositions.get(),
        }
    }

    /// Constructs a new `CanonicalCompositionBorrowed` using compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    #[expect(clippy::new_ret_no_self)]
    pub const fn new() -> CanonicalCompositionBorrowed<'static> {
        CanonicalCompositionBorrowed::new()
    }

    icu_provider::gen_buffer_data_constructors!(() -> error: DataError,
        functions: [
            new: skip,
            try_new_with_buffer_provider,
            try_new_unstable,
            Self,
        ]
    );

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new)]
    pub fn try_new_unstable<D>(provider: &D) -> Result<Self, DataError>
    where
        D: DataProvider<NormalizerNfcV1> + ?Sized,
    {
        let canonical_compositions: DataPayload<NormalizerNfcV1> =
            provider.load(Default::default())?.payload;
        Ok(CanonicalComposition {
            canonical_compositions,
        })
    }
}

/// The outcome of non-recursive canonical decomposition of a character.
#[allow(clippy::exhaustive_enums)]
#[derive(Debug, PartialEq, Eq)]
pub enum Decomposed {
    /// The character is its own canonical decomposition.
    Default,
    /// The character decomposes to a single different character.
    Singleton(char),
    /// The character decomposes to two characters.
    Expansion(char, char),
}

/// Borrowed version of the raw (non-recursive) canonical decomposition operation.
///
/// Callers should generally use `DecomposingNormalizer` instead of this API.
/// However, this API is provided for callers such as HarfBuzz that specifically
/// want access to non-recursive canonical decomposition e.g. for use in a
/// glyph-availability-guided custom normalizer.
#[derive(Debug)]
pub struct CanonicalDecompositionBorrowed<'a> {
    decompositions: &'a DecompositionData<'a>,
    tables: &'a DecompositionTables<'a>,
    non_recursive: &'a NonRecursiveDecompositionSupplement<'a>,
}

#[cfg(feature = "compiled_data")]
impl Default for CanonicalDecompositionBorrowed<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl CanonicalDecompositionBorrowed<'static> {
    /// Cheaply converts a [`CanonicalDecompositionBorrowed<'static>`] into a [`CanonicalDecomposition`].
    ///
    /// Note: Due to branching and indirection, using [`CanonicalDecomposition`] might inhibit some
    /// compile-time optimizations that are possible with [`CanonicalDecompositionBorrowed`].
    pub const fn static_to_owned(self) -> CanonicalDecomposition {
        CanonicalDecomposition {
            decompositions: DataPayload::from_static_ref(self.decompositions),
            tables: DataPayload::from_static_ref(self.tables),
            non_recursive: DataPayload::from_static_ref(self.non_recursive),
        }
    }

    /// Construct from compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub const fn new() -> Self {
        const _: () = assert!(
            crate::provider::Baked::SINGLETON_NORMALIZER_NFD_TABLES_V1
                .scalars16
                .const_len()
                + crate::provider::Baked::SINGLETON_NORMALIZER_NFD_TABLES_V1
                    .scalars24
                    .const_len()
                <= 0xFFF,
            "future extension"
        );

        Self {
            decompositions: crate::provider::Baked::SINGLETON_NORMALIZER_NFD_DATA_V1,
            tables: crate::provider::Baked::SINGLETON_NORMALIZER_NFD_TABLES_V1,
            non_recursive: crate::provider::Baked::SINGLETON_NORMALIZER_NFD_SUPPLEMENT_V1,
        }
    }
}

impl CanonicalDecompositionBorrowed<'_> {
    /// Performs non-recursive canonical decomposition (including for Hangul).
    ///
    /// ```
    ///     use icu::normalizer::properties::Decomposed;
    ///     let decomp = icu::normalizer::properties::CanonicalDecompositionBorrowed::new();
    ///
    ///     assert_eq!(decomp.decompose('e'), Decomposed::Default);
    ///     assert_eq!(
    ///         decomp.decompose('ệ'),
    ///         Decomposed::Expansion('ẹ', '\u{0302}')
    ///     );
    ///     assert_eq!(decomp.decompose('각'), Decomposed::Expansion('가', 'ᆨ'));
    ///     assert_eq!(decomp.decompose('\u{212B}'), Decomposed::Singleton('Å')); // ANGSTROM SIGN
    ///     assert_eq!(decomp.decompose('\u{2126}'), Decomposed::Singleton('Ω')); // OHM SIGN
    ///     assert_eq!(decomp.decompose('\u{1F71}'), Decomposed::Singleton('ά')); // oxia
    /// ```
    #[inline]
    pub fn decompose(&self, c: char) -> Decomposed {
        let lvt = u32::from(c).wrapping_sub(HANGUL_S_BASE);
        if lvt >= HANGUL_S_COUNT {
            return self.decompose_non_hangul(c);
        }
        // Invariant: lvt ≤ HANGUL_S_COUNT = 1172
        let t = lvt % HANGUL_T_COUNT;
        // Invariant: t ≤ (1172 / HANGUL_T_COUNT = 1172 / 28 = 41)
        if t == 0 {
            let l = lvt / HANGUL_N_COUNT;
            // Invariant: v ≤ (1172 / HANGUL_N_COUNT = 1172 / 588 ≈ 2)
            let v = (lvt % HANGUL_N_COUNT) / HANGUL_T_COUNT;
            // Invariant: v < (HANGUL_N_COUNT / HANGUL_T_COUNT = 588 / 28 = 21)
            return Decomposed::Expansion(
                // Safety: HANGUL_*_BASE are 0x1nnn, addding numbers that are 21 and 41
                // max will keep it in range, less than 0xD800
                unsafe { char::from_u32_unchecked(HANGUL_L_BASE + l) },
                unsafe { char::from_u32_unchecked(HANGUL_V_BASE + v) },
            );
        }
        let lv = lvt - t;
        // Invariant: lvt < 1172
        // Safe because values known to be in range
        Decomposed::Expansion(
            // Safety: HANGUL_*_BASE are 0x1nnn, addding numbers that are 1172 and 41
            // max will keep it in range, less than 0xD800
            unsafe { char::from_u32_unchecked(HANGUL_S_BASE + lv) },
            unsafe { char::from_u32_unchecked(HANGUL_T_BASE + t) },
        )
    }

    /// Performs non-recursive canonical decomposition except Hangul syllables
    /// are reported as `Decomposed::Default`.
    #[inline(always)]
    fn decompose_non_hangul(&self, c: char) -> Decomposed {
        let decomposition = self.decompositions.trie.get(c);
        // The REPLACEMENT CHARACTER has `NON_ROUND_TRIP_MARKER` set,
        // and that flag needs to be ignored here.
        if (decomposition & !(BACKWARD_COMBINING_MARKER | NON_ROUND_TRIP_MARKER)) == 0 {
            return Decomposed::Default;
        }
        // The loop is only broken out of as goto forward
        #[expect(clippy::never_loop)]
        loop {
            let high_zeros = (decomposition & HIGH_ZEROS_MASK) == 0;
            let low_zeros = (decomposition & LOW_ZEROS_MASK) == 0;
            if !high_zeros && !low_zeros {
                // Decomposition into two BMP characters: starter and non-starter
                if in_inclusive_range(c, '\u{1F71}', '\u{1FFB}') {
                    // Look in the other trie due to oxia singleton
                    // mappings to corresponding character with tonos.
                    break;
                }
                let starter = char_from_u32(decomposition & 0x7FFF);
                let combining = char_from_u32((decomposition >> 15) & 0x7FFF);
                return Decomposed::Expansion(starter, combining);
            }
            if high_zeros {
                // Decomposition into one BMP character or non-starter
                if trie_value_has_ccc(decomposition) {
                    // Non-starter
                    if !in_inclusive_range(c, '\u{0340}', '\u{0F81}') {
                        return Decomposed::Default;
                    }
                    return match c {
                        '\u{0340}' => {
                            // COMBINING GRAVE TONE MARK
                            Decomposed::Singleton('\u{0300}')
                        }
                        '\u{0341}' => {
                            // COMBINING ACUTE TONE MARK
                            Decomposed::Singleton('\u{0301}')
                        }
                        '\u{0343}' => {
                            // COMBINING GREEK KORONIS
                            Decomposed::Singleton('\u{0313}')
                        }
                        '\u{0344}' => {
                            // COMBINING GREEK DIALYTIKA TONOS
                            Decomposed::Expansion('\u{0308}', '\u{0301}')
                        }
                        '\u{0F73}' => {
                            // TIBETAN VOWEL SIGN II
                            Decomposed::Expansion('\u{0F71}', '\u{0F72}')
                        }
                        '\u{0F75}' => {
                            // TIBETAN VOWEL SIGN UU
                            Decomposed::Expansion('\u{0F71}', '\u{0F74}')
                        }
                        '\u{0F81}' => {
                            // TIBETAN VOWEL SIGN REVERSED II
                            Decomposed::Expansion('\u{0F71}', '\u{0F80}')
                        }
                        _ => Decomposed::Default,
                    };
                }
                let singleton = decomposition as u16;
                debug_assert_ne!(
                    singleton, FDFA_MARKER,
                    "How come we got the U+FDFA NFKD marker here?"
                );
                return Decomposed::Singleton(char_from_u16(singleton));
            }
            if c == '\u{212B}' {
                // ANGSTROM SIGN
                return Decomposed::Singleton('\u{00C5}');
            }
            // Only 12 of 14 bits used as of Unicode 16.
            let offset = (((decomposition & !(0b11 << 30)) >> 16) as usize) - 1;
            // Only 3 of 4 bits used as of Unicode 16.
            let len_bits = decomposition & 0b1111;
            let tables = self.tables;
            if offset < tables.scalars16.len() {
                if len_bits != 0 {
                    // i.e. logical len isn't 2
                    break;
                }
                if let Some(first) = tables.scalars16.get(offset) {
                    if let Some(second) = tables.scalars16.get(offset + 1) {
                        // Two BMP starters
                        return Decomposed::Expansion(char_from_u16(first), char_from_u16(second));
                    }
                }
                // GIGO case
                debug_assert!(false);
                return Decomposed::Default;
            }
            let len = len_bits + 1;
            if len > 2 {
                break;
            }
            let offset24 = offset - tables.scalars16.len();
            if let Some(first_c) = tables.scalars24.get(offset24) {
                if len == 1 {
                    return Decomposed::Singleton(first_c);
                }
                if let Some(second_c) = tables.scalars24.get(offset24 + 1) {
                    return Decomposed::Expansion(first_c, second_c);
                }
            }
            // GIGO case
            debug_assert!(false);
            return Decomposed::Default;
        }
        let non_recursive = self.non_recursive;
        let non_recursive_decomposition = non_recursive.trie.get(c);
        if non_recursive_decomposition == 0 {
            // GIGO case
            debug_assert!(false);
            return Decomposed::Default;
        }
        let trail_or_complex = (non_recursive_decomposition >> 16) as u16;
        let lead = non_recursive_decomposition as u16;
        if lead != 0 && trail_or_complex != 0 {
            // Decomposition into two BMP characters
            return Decomposed::Expansion(char_from_u16(lead), char_from_u16(trail_or_complex));
        }
        if lead != 0 {
            // Decomposition into one BMP character
            return Decomposed::Singleton(char_from_u16(lead));
        }
        // Decomposition into two non-BMP characters
        // Low is offset into a table plus one to keep it non-zero.
        let offset = usize::from(trail_or_complex - 1);
        if let Some(first) = non_recursive.scalars24.get(offset) {
            if let Some(second) = non_recursive.scalars24.get(offset + 1) {
                return Decomposed::Expansion(first, second);
            }
        }
        // GIGO case
        debug_assert!(false);
        Decomposed::Default
    }
}

/// The raw (non-recursive) canonical decomposition operation.
///
/// Callers should generally use `DecomposingNormalizer` instead of this API.
/// However, this API is provided for callers such as HarfBuzz that specifically
/// want access to non-recursive canonical decomposition e.g. for use in a
/// glyph-availability-guided custom normalizer.
#[derive(Debug)]
pub struct CanonicalDecomposition {
    decompositions: DataPayload<NormalizerNfdDataV1>,
    tables: DataPayload<NormalizerNfdTablesV1>,
    non_recursive: DataPayload<NormalizerNfdSupplementV1>,
}

#[cfg(feature = "compiled_data")]
impl Default for CanonicalDecomposition {
    fn default() -> Self {
        Self::new().static_to_owned()
    }
}

impl CanonicalDecomposition {
    /// Constructs a borrowed version of this type for more efficient querying.
    pub fn as_borrowed(&self) -> CanonicalDecompositionBorrowed<'_> {
        CanonicalDecompositionBorrowed {
            decompositions: self.decompositions.get(),
            tables: self.tables.get(),
            non_recursive: self.non_recursive.get(),
        }
    }

    /// Construct from compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    #[expect(clippy::new_ret_no_self)]
    pub const fn new() -> CanonicalDecompositionBorrowed<'static> {
        CanonicalDecompositionBorrowed::new()
    }

    icu_provider::gen_buffer_data_constructors!(() -> error: DataError,
        functions: [
            new: skip,
            try_new_with_buffer_provider,
            try_new_unstable,
            Self,
        ]
    );

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new)]
    pub fn try_new_unstable<D>(provider: &D) -> Result<Self, DataError>
    where
        D: DataProvider<NormalizerNfdDataV1>
            + DataProvider<NormalizerNfdTablesV1>
            + DataProvider<NormalizerNfdSupplementV1>
            + ?Sized,
    {
        let decompositions: DataPayload<NormalizerNfdDataV1> =
            provider.load(Default::default())?.payload;
        let tables: DataPayload<NormalizerNfdTablesV1> = provider.load(Default::default())?.payload;

        if tables.get().scalars16.len() + tables.get().scalars24.len() > 0xFFF {
            // The data is from a future where there exists a normalization flavor whose
            // complex decompositions take more than 0xFFF but fewer than 0x1FFF code points
            // of space. If a good use case from such a decomposition flavor arises, we can
            // dynamically change the bit masks so that the length mask becomes 0x1FFF instead
            // of 0xFFF and the all-non-starters mask becomes 0 instead of 0x1000. However,
            // since for now the masks are hard-coded, error out.
            return Err(DataError::custom("future extension"));
        }

        let non_recursive: DataPayload<NormalizerNfdSupplementV1> =
            provider.load(Default::default())?.payload;

        Ok(CanonicalDecomposition {
            decompositions,
            tables,
            non_recursive,
        })
    }
}

/// Borrowed version of lookup of the Canonical_Combining_Class Unicode property.
///
/// # Example
///
/// ```
/// use icu::properties::props::CanonicalCombiningClass;
/// use icu::normalizer::properties::CanonicalCombiningClassMapBorrowed;
///
/// let map = CanonicalCombiningClassMapBorrowed::new();
/// assert_eq!(map.get('a'), CanonicalCombiningClass::NotReordered); // U+0061: LATIN SMALL LETTER A
/// assert_eq!(map.get32(0x0301), CanonicalCombiningClass::Above); // U+0301: COMBINING ACUTE ACCENT
/// ```
#[derive(Debug)]
pub struct CanonicalCombiningClassMapBorrowed<'a> {
    /// The data trie
    decompositions: &'a DecompositionData<'a>,
}

#[cfg(feature = "compiled_data")]
impl Default for CanonicalCombiningClassMapBorrowed<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl CanonicalCombiningClassMapBorrowed<'static> {
    /// Cheaply converts a [`CanonicalCombiningClassMapBorrowed<'static>`] into a [`CanonicalCombiningClassMap`].
    ///
    /// Note: Due to branching and indirection, using [`CanonicalCombiningClassMap`] might inhibit some
    /// compile-time optimizations that are possible with [`CanonicalCombiningClassMapBorrowed`].
    pub const fn static_to_owned(self) -> CanonicalCombiningClassMap {
        CanonicalCombiningClassMap {
            decompositions: DataPayload::from_static_ref(self.decompositions),
        }
    }

    /// Construct from compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub const fn new() -> Self {
        CanonicalCombiningClassMapBorrowed {
            decompositions: crate::provider::Baked::SINGLETON_NORMALIZER_NFD_DATA_V1,
        }
    }
}

impl CanonicalCombiningClassMapBorrowed<'_> {
    /// Look up the canonical combining class for a scalar value.
    ///
    /// The return value is a u8 representing the canonical combining class,
    /// you may enable the `"icu_properties"` feature if you would like to use a typed
    /// `CanonicalCombiningClass`.
    #[inline(always)]
    pub fn get_u8(&self, c: char) -> u8 {
        self.get32_u8(u32::from(c))
    }

    /// Look up the canonical combining class for a scalar value
    /// represented as `u32`. If the argument is outside the scalar
    /// value range, `Not_Reordered` is returned.
    ///
    /// The return value is a u8 representing the canonical combining class,
    /// you may enable the `"icu_properties"` feature if you would like to use a typed
    /// `CanonicalCombiningClass`.
    pub fn get32_u8(&self, c: u32) -> u8 {
        let trie_value = self.decompositions.trie.get32(c);
        if trie_value_has_ccc(trie_value) {
            trie_value as u8
        } else {
            ccc!(NotReordered, 0).to_icu4c_value()
        }
    }

    /// Look up the canonical combining class for a scalar value
    ///
    /// ✨ *Enabled with the `icu_properties` Cargo feature.*
    #[inline(always)]
    #[cfg(feature = "icu_properties")]
    pub fn get(&self, c: char) -> CanonicalCombiningClass {
        CanonicalCombiningClass::from_icu4c_value(self.get_u8(c))
    }

    /// Look up the canonical combining class for a scalar value
    /// represented as `u32`. If the argument is outside the scalar
    /// value range, `CanonicalCombiningClass::NotReordered` is returned.
    ///
    /// ✨ *Enabled with the `icu_properties` Cargo feature.*
    #[cfg(feature = "icu_properties")]
    pub fn get32(&self, c: u32) -> CanonicalCombiningClass {
        CanonicalCombiningClass::from_icu4c_value(self.get32_u8(c))
    }
}

/// Lookup of the Canonical_Combining_Class Unicode property.
#[derive(Debug)]
pub struct CanonicalCombiningClassMap {
    /// The data trie
    decompositions: DataPayload<NormalizerNfdDataV1>,
}

#[cfg(feature = "compiled_data")]
impl Default for CanonicalCombiningClassMap {
    fn default() -> Self {
        Self::new().static_to_owned()
    }
}

impl CanonicalCombiningClassMap {
    /// Constructs a borrowed version of this type for more efficient querying.
    pub fn as_borrowed(&self) -> CanonicalCombiningClassMapBorrowed<'_> {
        CanonicalCombiningClassMapBorrowed {
            decompositions: self.decompositions.get(),
        }
    }

    /// Construct from compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    #[expect(clippy::new_ret_no_self)]
    pub const fn new() -> CanonicalCombiningClassMapBorrowed<'static> {
        CanonicalCombiningClassMapBorrowed::new()
    }

    icu_provider::gen_buffer_data_constructors!(() -> error: DataError,
        functions: [
            new: skip,
            try_new_with_buffer_provider,
            try_new_unstable,
            Self,
    ]);

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new)]
    pub fn try_new_unstable<D>(provider: &D) -> Result<Self, DataError>
    where
        D: DataProvider<NormalizerNfdDataV1> + ?Sized,
    {
        let decompositions: DataPayload<NormalizerNfdDataV1> =
            provider.load(Default::default())?.payload;
        Ok(CanonicalCombiningClassMap { decompositions })
    }
}
