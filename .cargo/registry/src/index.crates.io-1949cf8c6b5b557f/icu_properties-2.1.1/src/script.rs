// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Data and APIs for supporting Script_Extensions property
//! values in an efficient structure.

use crate::props::Script;
use crate::provider::*;

#[cfg(feature = "alloc")]
use core::iter::FromIterator;
use core::ops::RangeInclusive;
#[cfg(feature = "alloc")]
use icu_collections::codepointinvlist::CodePointInversionList;
use icu_provider::prelude::*;
use zerovec::{ule::AsULE, ZeroSlice};

/// The number of bits at the low-end of a `ScriptWithExt` value used for
/// storing the `Script` value (or `extensions` index).
const SCRIPT_VAL_LENGTH: u16 = 10;

/// The bit mask necessary to retrieve the `Script` value (or `extensions` index)
/// from a `ScriptWithExt` value.
const SCRIPT_X_SCRIPT_VAL: u16 = (1 << SCRIPT_VAL_LENGTH) - 1;

/// An internal-use only pseudo-property that represents the values stored in
/// the trie of the special data structure [`ScriptWithExtensionsProperty`].
///
/// Note: The will assume a 12-bit layout. The 2 higher order bits in positions
/// 11..10 will indicate how to deduce the Script value and Script_Extensions,
/// and the lower 10 bits 9..0 indicate either the Script value or the index
/// into the `extensions` structure.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "datagen", derive(databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_properties::script))]
#[repr(transparent)]
#[doc(hidden)]
// `ScriptWithExt` not intended as public-facing but for `ScriptWithExtensionsProperty` constructor
#[allow(clippy::exhaustive_structs)] // this type is stable
pub struct ScriptWithExt(pub u16);

#[allow(missing_docs)] // These constants don't need individual documentation.
#[allow(non_upper_case_globals)]
#[doc(hidden)] // `ScriptWithExt` not intended as public-facing but for `ScriptWithExtensionsProperty` constructor
impl ScriptWithExt {
    pub const Unknown: ScriptWithExt = ScriptWithExt(0);
}

impl AsULE for ScriptWithExt {
    type ULE = <u16 as AsULE>::ULE;

    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        Script(self.0).to_unaligned()
    }

    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        ScriptWithExt(Script::from_unaligned(unaligned).0)
    }
}

#[doc(hidden)] // `ScriptWithExt` not intended as public-facing but for `ScriptWithExtensionsProperty` constructor
impl ScriptWithExt {
    /// Returns whether the [`ScriptWithExt`] value has Script_Extensions and
    /// also indicates a Script value of [`Script::Common`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::script::ScriptWithExt;
    ///
    /// assert!(ScriptWithExt(0x04FF).is_common());
    /// assert!(ScriptWithExt(0x0400).is_common());
    ///
    /// assert!(!ScriptWithExt(0x08FF).is_common());
    /// assert!(!ScriptWithExt(0x0800).is_common());
    ///
    /// assert!(!ScriptWithExt(0x0CFF).is_common());
    /// assert!(!ScriptWithExt(0x0C00).is_common());
    ///
    /// assert!(!ScriptWithExt(0xFF).is_common());
    /// assert!(!ScriptWithExt(0x0).is_common());
    /// ```
    pub fn is_common(&self) -> bool {
        self.0 >> SCRIPT_VAL_LENGTH == 1
    }

    /// Returns whether the [`ScriptWithExt`] value has Script_Extensions and
    /// also indicates a Script value of [`Script::Inherited`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::script::ScriptWithExt;
    ///
    /// assert!(!ScriptWithExt(0x04FF).is_inherited());
    /// assert!(!ScriptWithExt(0x0400).is_inherited());
    ///
    /// assert!(ScriptWithExt(0x08FF).is_inherited());
    /// assert!(ScriptWithExt(0x0800).is_inherited());
    ///
    /// assert!(!ScriptWithExt(0x0CFF).is_inherited());
    /// assert!(!ScriptWithExt(0x0C00).is_inherited());
    ///
    /// assert!(!ScriptWithExt(0xFF).is_inherited());
    /// assert!(!ScriptWithExt(0x0).is_inherited());
    /// ```
    pub fn is_inherited(&self) -> bool {
        self.0 >> SCRIPT_VAL_LENGTH == 2
    }

    /// Returns whether the [`ScriptWithExt`] value has Script_Extensions and
    /// also indicates that the Script value is neither [`Script::Common`] nor
    /// [`Script::Inherited`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::script::ScriptWithExt;
    ///
    /// assert!(!ScriptWithExt(0x04FF).is_other());
    /// assert!(!ScriptWithExt(0x0400).is_other());
    ///
    /// assert!(!ScriptWithExt(0x08FF).is_other());
    /// assert!(!ScriptWithExt(0x0800).is_other());
    ///
    /// assert!(ScriptWithExt(0x0CFF).is_other());
    /// assert!(ScriptWithExt(0x0C00).is_other());
    ///
    /// assert!(!ScriptWithExt(0xFF).is_other());
    /// assert!(!ScriptWithExt(0x0).is_other());
    /// ```
    pub fn is_other(&self) -> bool {
        self.0 >> SCRIPT_VAL_LENGTH == 3
    }

    /// Returns whether the [`ScriptWithExt`] value has Script_Extensions.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::script::ScriptWithExt;
    ///
    /// assert!(ScriptWithExt(0x04FF).has_extensions());
    /// assert!(ScriptWithExt(0x0400).has_extensions());
    ///
    /// assert!(ScriptWithExt(0x08FF).has_extensions());
    /// assert!(ScriptWithExt(0x0800).has_extensions());
    ///
    /// assert!(ScriptWithExt(0x0CFF).has_extensions());
    /// assert!(ScriptWithExt(0x0C00).has_extensions());
    ///
    /// assert!(!ScriptWithExt(0xFF).has_extensions());
    /// assert!(!ScriptWithExt(0x0).has_extensions());
    /// ```
    pub fn has_extensions(&self) -> bool {
        let high_order_bits = self.0 >> SCRIPT_VAL_LENGTH;
        high_order_bits > 0
    }
}

impl From<ScriptWithExt> for u32 {
    fn from(swe: ScriptWithExt) -> Self {
        swe.0 as u32
    }
}

impl From<ScriptWithExt> for Script {
    fn from(swe: ScriptWithExt) -> Self {
        Script(swe.0)
    }
}

/// A struct that wraps a [`Script`] array, such as in the return value for
/// [`get_script_extensions_val()`](ScriptWithExtensionsBorrowed::get_script_extensions_val).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ScriptExtensionsSet<'a> {
    values: &'a ZeroSlice<Script>,
}

impl<'a> ScriptExtensionsSet<'a> {
    /// Returns whether this set contains the given script.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::props::Script;
    /// use icu::properties::script::ScriptWithExtensions;
    /// let swe = ScriptWithExtensions::new();
    ///
    /// assert!(swe
    ///     .get_script_extensions_val('\u{11303}') // GRANTHA SIGN VISARGA
    ///     .contains(&Script::Grantha));
    /// ```
    pub fn contains(&self, x: &Script) -> bool {
        ZeroSlice::binary_search(self.values, x).is_ok()
    }

    /// Gets an iterator over the elements.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::props::Script;
    /// use icu::properties::script::ScriptWithExtensions;
    /// let swe = ScriptWithExtensions::new();
    ///
    /// assert_eq!(
    ///     swe.get_script_extensions_val('௫') // U+0BEB TAMIL DIGIT FIVE
    ///         .iter()
    ///         .collect::<Vec<_>>(),
    ///     [Script::Tamil, Script::Grantha]
    /// );
    /// ```
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = Script> + 'a {
        ZeroSlice::iter(self.values)
    }

    /// For accessing this set as an array instead of an iterator
    #[doc(hidden)] // used by FFI code
    pub fn array_len(&self) -> usize {
        self.values.len()
    }
    /// For accessing this set as an array instead of an iterator
    #[doc(hidden)] // used by FFI code
    pub fn array_get(&self, index: usize) -> Option<Script> {
        self.values.get(index)
    }
}

/// A struct that represents the data for the Script and Script_Extensions properties.
///
/// ✨ *Enabled with the `compiled_data` Cargo feature.*
///
/// [📚 Help choosing a constructor](icu_provider::constructors)
///
/// Most useful methods are on [`ScriptWithExtensionsBorrowed`] obtained by calling [`ScriptWithExtensions::as_borrowed()`]
///
/// # Examples
///
/// ```
/// use icu::properties::script::ScriptWithExtensions;
/// use icu::properties::props::Script;
/// let swe = ScriptWithExtensions::new();
///
/// // get the `Script` property value
/// assert_eq!(swe.get_script_val('ـ'), Script::Common); // U+0640 ARABIC TATWEEL
/// assert_eq!(swe.get_script_val('\u{0650}'), Script::Inherited); // U+0650 ARABIC KASRA
/// assert_eq!(swe.get_script_val('٠'), Script::Arabic); // // U+0660 ARABIC-INDIC DIGIT ZERO
/// assert_eq!(swe.get_script_val('ﷲ'), Script::Arabic); // U+FDF2 ARABIC LIGATURE ALLAH ISOLATED FORM
///
/// // get the `Script_Extensions` property value
/// assert_eq!(
///     swe.get_script_extensions_val('ـ') // U+0640 ARABIC TATWEEL
///         .iter().collect::<Vec<_>>(),
///     [Script::Arabic, Script::Syriac, Script::Mandaic, Script::Manichaean,
///          Script::PsalterPahlavi, Script::Adlam, Script::HanifiRohingya, Script::Sogdian,
///          Script::OldUyghur]
/// );
/// assert_eq!(
///     swe.get_script_extensions_val('🥳') // U+1F973 FACE WITH PARTY HORN AND PARTY HAT
///         .iter().collect::<Vec<_>>(),
///     [Script::Common]
/// );
/// assert_eq!(
///     swe.get_script_extensions_val('\u{200D}') // ZERO WIDTH JOINER
///         .iter().collect::<Vec<_>>(),
///     [Script::Inherited]
/// );
/// assert_eq!(
///     swe.get_script_extensions_val('௫') // U+0BEB TAMIL DIGIT FIVE
///         .iter().collect::<Vec<_>>(),
///     [Script::Tamil, Script::Grantha]
/// );
///
/// // check containment of a `Script` value in the `Script_Extensions` value
/// // U+0650 ARABIC KASRA
/// assert!(!swe.has_script('\u{0650}', Script::Inherited)); // main Script value
/// assert!(swe.has_script('\u{0650}', Script::Arabic));
/// assert!(swe.has_script('\u{0650}', Script::Syriac));
/// assert!(!swe.has_script('\u{0650}', Script::Thaana));
///
/// // get a `CodePointInversionList` for when `Script` value is contained in `Script_Extensions` value
/// let syriac = swe.get_script_extensions_set(Script::Syriac);
/// assert!(syriac.contains('\u{0650}')); // ARABIC KASRA
/// assert!(!syriac.contains('٠')); // ARABIC-INDIC DIGIT ZERO
/// assert!(!syriac.contains('ﷲ')); // ARABIC LIGATURE ALLAH ISOLATED FORM
/// assert!(syriac.contains('܀')); // SYRIAC END OF PARAGRAPH
/// assert!(syriac.contains('\u{074A}')); // SYRIAC BARREKH
/// ```
#[derive(Debug)]
pub struct ScriptWithExtensions {
    data: DataPayload<PropertyScriptWithExtensionsV1>,
}

/// A borrowed wrapper around script extension data, returned by
/// [`ScriptWithExtensions::as_borrowed()`]. More efficient to query.
#[derive(Clone, Copy, Debug)]
pub struct ScriptWithExtensionsBorrowed<'a> {
    data: &'a ScriptWithExtensionsProperty<'a>,
}

impl ScriptWithExtensions {
    /// Creates a new instance of `ScriptWithExtensionsBorrowed` using compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    #[expect(clippy::new_ret_no_self)]
    pub fn new() -> ScriptWithExtensionsBorrowed<'static> {
        ScriptWithExtensionsBorrowed::new()
    }

    icu_provider::gen_buffer_data_constructors!(
        () -> result: Result<ScriptWithExtensions, DataError>,
        functions: [
            new: skip,
            try_new_with_buffer_provider,
            try_new_unstable,
            Self,
        ]
    );

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new)]
    pub fn try_new_unstable(
        provider: &(impl DataProvider<PropertyScriptWithExtensionsV1> + ?Sized),
    ) -> Result<Self, DataError> {
        Ok(ScriptWithExtensions::from_data(
            provider.load(Default::default())?.payload,
        ))
    }

    /// Construct a borrowed version of this type that can be queried.
    ///
    /// This avoids a potential small underlying cost per API call (ex: `contains()`) by consolidating it
    /// up front.
    #[inline]
    pub fn as_borrowed(&self) -> ScriptWithExtensionsBorrowed<'_> {
        ScriptWithExtensionsBorrowed {
            data: self.data.get(),
        }
    }

    /// Construct a new one from loaded data
    ///
    /// Typically it is preferable to use getters like [`load_script_with_extensions_unstable()`] instead
    pub(crate) fn from_data(data: DataPayload<PropertyScriptWithExtensionsV1>) -> Self {
        Self { data }
    }
}

impl<'a> ScriptWithExtensionsBorrowed<'a> {
    /// Returns the `Script` property value for this code point.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::script::ScriptWithExtensions;
    /// use icu::properties::props::Script;
    ///
    /// let swe = ScriptWithExtensions::new();
    ///
    /// // U+0640 ARABIC TATWEEL
    /// assert_eq!(swe.get_script_val('ـ'), Script::Common); // main Script value
    /// assert_ne!(swe.get_script_val('ـ'), Script::Arabic);
    /// assert_ne!(swe.get_script_val('ـ'), Script::Syriac);
    /// assert_ne!(swe.get_script_val('ـ'), Script::Thaana);
    ///
    /// // U+0650 ARABIC KASRA
    /// assert_eq!(swe.get_script_val('\u{0650}'), Script::Inherited); // main Script value
    /// assert_ne!(swe.get_script_val('\u{0650}'), Script::Arabic);
    /// assert_ne!(swe.get_script_val('\u{0650}'), Script::Syriac);
    /// assert_ne!(swe.get_script_val('\u{0650}'), Script::Thaana);
    ///
    /// // U+0660 ARABIC-INDIC DIGIT ZERO
    /// assert_ne!(swe.get_script_val('٠'), Script::Common);
    /// assert_eq!(swe.get_script_val('٠'), Script::Arabic); // main Script value
    /// assert_ne!(swe.get_script_val('٠'), Script::Syriac);
    /// assert_ne!(swe.get_script_val('٠'), Script::Thaana);
    ///
    /// // U+FDF2 ARABIC LIGATURE ALLAH ISOLATED FORM
    /// assert_ne!(swe.get_script_val('ﷲ'), Script::Common);
    /// assert_eq!(swe.get_script_val('ﷲ'), Script::Arabic); // main Script value
    /// assert_ne!(swe.get_script_val('ﷲ'), Script::Syriac);
    /// assert_ne!(swe.get_script_val('ﷲ'), Script::Thaana);
    /// ```
    pub fn get_script_val(self, ch: char) -> Script {
        self.get_script_val32(ch as u32)
    }

    /// See [`Self::get_script_val`].
    pub fn get_script_val32(self, code_point: u32) -> Script {
        let sc_with_ext = self.data.trie.get32(code_point);

        if sc_with_ext.is_other() {
            let ext_idx = sc_with_ext.0 & SCRIPT_X_SCRIPT_VAL;
            let scx_val = self.data.extensions.get(ext_idx as usize);
            let scx_first_sc = scx_val.and_then(|scx| scx.get(0));

            let default_sc_val = Script::Unknown;

            scx_first_sc.unwrap_or(default_sc_val)
        } else if sc_with_ext.is_common() {
            Script::Common
        } else if sc_with_ext.is_inherited() {
            Script::Inherited
        } else {
            let script_val = sc_with_ext.0;
            Script(script_val)
        }
    }
    // Returns the Script_Extensions value for a code_point when the trie value
    // is already known.
    // This private helper method exists to prevent code duplication in callers like
    // `get_script_extensions_val`, `get_script_extensions_set`, and `has_script`.
    fn get_scx_val_using_trie_val(
        self,
        sc_with_ext_ule: &'a <ScriptWithExt as AsULE>::ULE,
    ) -> &'a ZeroSlice<Script> {
        let sc_with_ext = ScriptWithExt::from_unaligned(*sc_with_ext_ule);
        if sc_with_ext.is_other() {
            let ext_idx = sc_with_ext.0 & SCRIPT_X_SCRIPT_VAL;
            let ext_subarray = self.data.extensions.get(ext_idx as usize);
            // In the OTHER case, where the 2 higher-order bits of the
            // `ScriptWithExt` value in the trie doesn't indicate the Script value,
            // the Script value is copied/inserted into the first position of the
            // `extensions` array. So we must remove it to return the actual scx array val.
            let scx_slice = ext_subarray
                .and_then(|zslice| zslice.as_ule_slice().get(1..))
                .unwrap_or_default();
            ZeroSlice::from_ule_slice(scx_slice)
        } else if sc_with_ext.is_common() || sc_with_ext.is_inherited() {
            let ext_idx = sc_with_ext.0 & SCRIPT_X_SCRIPT_VAL;
            let scx_val = self.data.extensions.get(ext_idx as usize);
            scx_val.unwrap_or_default()
        } else {
            // Note: `Script` and `ScriptWithExt` are both represented as the same
            // u16 value when the `ScriptWithExt` has no higher-order bits set.
            let script_ule_slice = core::slice::from_ref(sc_with_ext_ule);
            ZeroSlice::from_ule_slice(script_ule_slice)
        }
    }
    /// Return the `Script_Extensions` property value for this code point.
    ///
    /// If `code_point` has Script_Extensions, then return the Script codes in
    /// the Script_Extensions. In this case, the Script property value
    /// (normally Common or Inherited) is not included in the [`ScriptExtensionsSet`].
    ///
    /// If c does not have Script_Extensions, then the one Script code is put
    /// into the [`ScriptExtensionsSet`] and also returned.
    ///
    /// If c is not a valid code point, then return an empty [`ScriptExtensionsSet`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::script::ScriptWithExtensions;
    /// use icu::properties::props::Script;
    ///
    /// let swe = ScriptWithExtensions::new();
    ///
    /// assert_eq!(
    ///     swe.get_script_extensions_val('𐓐') // U+104D0 OSAGE CAPITAL LETTER KHA
    ///         .iter()
    ///         .collect::<Vec<_>>(),
    ///     [Script::Osage]
    /// );
    /// assert_eq!(
    ///     swe.get_script_extensions_val('🥳') // U+1F973 FACE WITH PARTY HORN AND PARTY HAT
    ///         .iter()
    ///         .collect::<Vec<_>>(),
    ///     [Script::Common]
    /// );
    /// assert_eq!(
    ///     swe.get_script_extensions_val('\u{200D}') // ZERO WIDTH JOINER
    ///         .iter()
    ///         .collect::<Vec<_>>(),
    ///     [Script::Inherited]
    /// );
    /// assert_eq!(
    ///     swe.get_script_extensions_val('௫') // U+0BEB TAMIL DIGIT FIVE
    ///         .iter()
    ///         .collect::<Vec<_>>(),
    ///     [Script::Tamil, Script::Grantha]
    /// );
    /// ```
    pub fn get_script_extensions_val(self, ch: char) -> ScriptExtensionsSet<'a> {
        self.get_script_extensions_val32(ch as u32)
    }

    /// See [`Self::get_script_extensions_val`].
    pub fn get_script_extensions_val32(self, code_point: u32) -> ScriptExtensionsSet<'a> {
        let sc_with_ext_ule = self.data.trie.get32_ule(code_point);

        ScriptExtensionsSet {
            values: match sc_with_ext_ule {
                Some(ule_ref) => self.get_scx_val_using_trie_val(ule_ref),
                None => ZeroSlice::from_ule_slice(&[]),
            },
        }
    }

    /// Returns whether `script` is contained in the Script_Extensions
    /// property value if the code_point has Script_Extensions, otherwise
    /// if the code point does not have Script_Extensions then returns
    /// whether the Script property value matches.
    ///
    /// Some characters are commonly used in multiple scripts. For more information,
    /// see UAX #24: <http://www.unicode.org/reports/tr24/>.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::script::ScriptWithExtensions;
    /// use icu::properties::props::Script;
    ///
    /// let swe = ScriptWithExtensions::new();
    ///
    /// // U+0650 ARABIC KASRA
    /// assert!(!swe.has_script('\u{0650}', Script::Inherited)); // main Script value
    /// assert!(swe.has_script('\u{0650}', Script::Arabic));
    /// assert!(swe.has_script('\u{0650}', Script::Syriac));
    /// assert!(!swe.has_script('\u{0650}', Script::Thaana));
    ///
    /// // U+0660 ARABIC-INDIC DIGIT ZERO
    /// assert!(!swe.has_script('٠', Script::Common)); // main Script value
    /// assert!(swe.has_script('٠', Script::Arabic));
    /// assert!(!swe.has_script('٠', Script::Syriac));
    /// assert!(swe.has_script('٠', Script::Thaana));
    ///
    /// // U+FDF2 ARABIC LIGATURE ALLAH ISOLATED FORM
    /// assert!(!swe.has_script('ﷲ', Script::Common));
    /// assert!(swe.has_script('ﷲ', Script::Arabic)); // main Script value
    /// assert!(!swe.has_script('ﷲ', Script::Syriac));
    /// assert!(swe.has_script('ﷲ', Script::Thaana));
    /// ```
    pub fn has_script(self, ch: char, script: Script) -> bool {
        self.has_script32(ch as u32, script)
    }

    /// See [`Self::has_script`].
    pub fn has_script32(self, code_point: u32, script: Script) -> bool {
        let sc_with_ext_ule = if let Some(scwe_ule) = self.data.trie.get32_ule(code_point) {
            scwe_ule
        } else {
            return false;
        };
        let sc_with_ext = <ScriptWithExt as AsULE>::from_unaligned(*sc_with_ext_ule);

        if !sc_with_ext.has_extensions() {
            let script_val = sc_with_ext.0;
            script == Script(script_val)
        } else {
            let scx_val = self.get_scx_val_using_trie_val(sc_with_ext_ule);
            let script_find = scx_val.iter().find(|&sc| sc == script);
            script_find.is_some()
        }
    }

    /// Returns all of the matching `CodePointMapRange`s for the given [`Script`]
    /// in which `has_script` will return true for all of the contained code points.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::props::Script;
    /// use icu::properties::script::ScriptWithExtensions;
    ///
    /// let swe = ScriptWithExtensions::new();
    ///
    /// let syriac_script_extensions_ranges =
    ///     swe.get_script_extensions_ranges(Script::Syriac);
    ///
    /// let exp_ranges = [
    ///     0x0303..=0x0304, // COMBINING TILDE..COMBINING MACRON
    ///     0x0307..=0x0308, // COMBINING DOT ABOVE..COMBINING DIAERESIS
    ///     0x030A..=0x030A, // COMBINING RING ABOVE
    ///     0x0323..=0x0325, // COMBINING DOT BELOW..COMBINING RING BELOW
    ///     0x032D..=0x032E, // COMBINING CIRCUMFLEX ACCENT BELOW..COMBINING BREVE BELOW
    ///     0x0330..=0x0331, // COMBINING TILDE BELOW..COMBINING MACRON BELOW
    ///     0x060C..=0x060C, // ARABIC COMMA
    ///     0x061B..=0x061C, // ARABIC SEMICOLON, ARABIC LETTER MARK
    ///     0x061F..=0x061F, // ARABIC QUESTION MARK
    ///     0x0640..=0x0640, // ARABIC TATWEEL
    ///     0x064B..=0x0655, // ARABIC FATHATAN..ARABIC HAMZA BELOW
    ///     0x0670..=0x0670, // ARABIC LETTER SUPERSCRIPT ALEF
    ///     0x0700..=0x070D, // Syriac block begins at U+0700
    ///     0x070F..=0x074A, // Syriac block
    ///     0x074D..=0x074F, // Syriac block ends at U+074F
    ///     0x0860..=0x086A, // Syriac Supplement block is U+0860..=U+086F
    ///     0x1DF8..=0x1DF8, // COMBINING DOT ABOVE LEFT
    ///     0x1DFA..=0x1DFA, // COMBINING DOT BELOW LEFT
    /// ];
    ///
    /// assert_eq!(
    ///     syriac_script_extensions_ranges.collect::<Vec<_>>(),
    ///     exp_ranges
    /// );
    /// ```
    pub fn get_script_extensions_ranges(
        self,
        script: Script,
    ) -> impl Iterator<Item = RangeInclusive<u32>> + 'a {
        self.data
            .trie
            .iter_ranges_mapped(move |value| {
                let sc_with_ext = ScriptWithExt(value.0);
                if sc_with_ext.has_extensions() {
                    self.get_scx_val_using_trie_val(&sc_with_ext.to_unaligned())
                        .iter()
                        .any(|sc| sc == script)
                } else {
                    script == sc_with_ext.into()
                }
            })
            .filter(|v| v.value)
            .map(|v| v.range)
    }

    /// Returns a [`CodePointInversionList`] for the given [`Script`] which represents all
    /// code points for which `has_script` will return true.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::script::ScriptWithExtensions;
    /// use icu::properties::props::Script;
    ///
    /// let swe = ScriptWithExtensions::new();
    ///
    /// let syriac = swe.get_script_extensions_set(Script::Syriac);
    ///
    /// assert!(!syriac.contains('؞')); // ARABIC TRIPLE DOT PUNCTUATION MARK
    /// assert!(syriac.contains('؟')); // ARABIC QUESTION MARK
    /// assert!(!syriac.contains('ؠ')); // ARABIC LETTER KASHMIRI YEH
    ///
    /// assert!(syriac.contains('܀')); // SYRIAC END OF PARAGRAPH
    /// assert!(syriac.contains('\u{074A}')); // SYRIAC BARREKH
    /// assert!(!syriac.contains('\u{074B}')); // unassigned
    /// assert!(syriac.contains('ݏ')); // SYRIAC LETTER SOGDIAN FE
    /// assert!(!syriac.contains('ݐ')); // ARABIC LETTER BEH WITH THREE DOTS HORIZONTALLY BELOW
    ///
    /// assert!(syriac.contains('\u{1DF8}')); // COMBINING DOT ABOVE LEFT
    /// assert!(!syriac.contains('\u{1DF9}')); // COMBINING WIDE INVERTED BRIDGE BELOW
    /// assert!(syriac.contains('\u{1DFA}')); // COMBINING DOT BELOW LEFT
    /// assert!(!syriac.contains('\u{1DFB}')); // COMBINING DELETION MARK
    /// ```
    #[cfg(feature = "alloc")]
    pub fn get_script_extensions_set(self, script: Script) -> CodePointInversionList<'a> {
        CodePointInversionList::from_iter(self.get_script_extensions_ranges(script))
    }
}

#[cfg(feature = "compiled_data")]
impl Default for ScriptWithExtensionsBorrowed<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptWithExtensionsBorrowed<'static> {
    /// Creates a new instance of `ScriptWithExtensionsBorrowed` using compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub fn new() -> Self {
        Self {
            data: crate::provider::Baked::SINGLETON_PROPERTY_SCRIPT_WITH_EXTENSIONS_V1,
        }
    }

    /// Cheaply converts a [`ScriptWithExtensionsBorrowed<'static>`] into a [`ScriptWithExtensions`].
    ///
    /// Note: Due to branching and indirection, using [`ScriptWithExtensions`] might inhibit some
    /// compile-time optimizations that are possible with [`ScriptWithExtensionsBorrowed`].
    pub const fn static_to_owned(self) -> ScriptWithExtensions {
        ScriptWithExtensions {
            data: DataPayload::from_static_ref(self.data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Regression test for https://github.com/unicode-org/icu4x/issues/6041
    fn test_scx_regression_6041() {
        let scripts = ScriptWithExtensions::new()
            .get_script_extensions_val('\u{2bc}')
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(
            scripts,
            [
                Script::Bengali,
                Script::Cyrillic,
                Script::Devanagari,
                Script::Latin,
                Script::Thai,
                Script::Lisu,
                Script::Toto
            ]
        );
    }
}
