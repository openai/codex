// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Data stored as as [`ZeroTrieSimpleAscii`]

/// This is a valid separator as `DataLocale` will never produce it.
///
/// Mostly for internal use
pub const ID_SEPARATOR: u8 = 0x1E;

pub use crate::DynamicDataMarker;
use crate::{
    prelude::{zerofrom::ZeroFrom, *},
    ule::MaybeAsVarULE,
};
pub use zerotrie::ZeroTrieSimpleAscii;
use zerovec::VarZeroSlice;

fn get_index(
    trie: ZeroTrieSimpleAscii<&'static [u8]>,
    id: DataIdentifierBorrowed,
    attributes_prefix_match: bool,
) -> Option<usize> {
    use writeable::Writeable;
    let mut cursor = trie.cursor();
    let _is_ascii = id.locale.write_to(&mut cursor);
    if !id.marker_attributes.is_empty() {
        cursor.step(ID_SEPARATOR);
        id.marker_attributes.write_to(&mut cursor).ok()?;
        loop {
            if let Some(v) = cursor.take_value() {
                break Some(v);
            }
            if !attributes_prefix_match || cursor.probe(0).is_none() {
                break None;
            }
        }
    } else {
        cursor.take_value()
    }
}

#[cfg(feature = "alloc")]
#[expect(clippy::type_complexity)]
fn iter(
    trie: &'static ZeroTrieSimpleAscii<&'static [u8]>,
) -> core::iter::FilterMap<
    zerotrie::ZeroTrieStringIterator<'static>,
    fn((alloc::string::String, usize)) -> Option<DataIdentifierCow<'static>>,
> {
    use alloc::borrow::ToOwned;
    trie.iter().filter_map(move |(s, _)| {
        if let Some((locale, attrs)) = s.split_once(ID_SEPARATOR as char) {
            Some(DataIdentifierCow::from_owned(
                DataMarkerAttributes::try_from_str(attrs).ok()?.to_owned(),
                locale.parse().ok()?,
            ))
        } else {
            s.parse().ok().map(DataIdentifierCow::from_locale)
        }
    })
}

/// Regular baked data: a trie for lookups and a slice of values
#[derive(Debug)]
pub struct Data<M: DataMarker> {
    // Unsafe invariant: actual values contained MUST be valid indices into `values`
    trie: ZeroTrieSimpleAscii<&'static [u8]>,
    values: &'static [M::DataStruct],
}

impl<M: DataMarker> Data<M> {
    /// Construct from a trie and values
    ///
    /// # Safety
    /// The actual values contained in the trie must be valid indices into `values`
    pub const unsafe fn from_trie_and_values_unchecked(
        trie: ZeroTrieSimpleAscii<&'static [u8]>,
        values: &'static [M::DataStruct],
    ) -> Self {
        Self { trie, values }
    }
}

impl<M: DataMarker> super::private::Sealed for Data<M> {}
impl<M: DataMarker> super::DataStore<M> for Data<M> {
    fn get(
        &self,
        id: DataIdentifierBorrowed,
        attributes_prefix_match: bool,
    ) -> Option<DataPayload<M>> {
        get_index(self.trie, id, attributes_prefix_match)
            // Safety: Allowed since `i` came from the trie and the field safety invariant
            .map(|i| unsafe { self.values.get_unchecked(i) })
            .map(DataPayload::from_static_ref)
    }

    #[cfg(feature = "alloc")]
    type IterReturn = core::iter::FilterMap<
        zerotrie::ZeroTrieStringIterator<'static>,
        fn((alloc::string::String, usize)) -> Option<DataIdentifierCow<'static>>,
    >;
    #[cfg(feature = "alloc")]
    fn iter(&'static self) -> Self::IterReturn {
        iter(&self.trie)
    }
}

/// Regular baked data: a trie for lookups and a slice of values
#[derive(Debug)]
pub struct DataRef<M: DataMarker> {
    // Unsafe invariant: actual values contained MUST be valid indices into `values`
    trie: ZeroTrieSimpleAscii<&'static [u8]>,
    values: &'static [&'static M::DataStruct],
}

impl<M: DataMarker> DataRef<M> {
    /// Construct from a trie and references to values
    ///
    /// # Safety
    /// The actual values contained in the trie must be valid indices into `values`
    pub const unsafe fn from_trie_and_refs_unchecked(
        trie: ZeroTrieSimpleAscii<&'static [u8]>,
        values: &'static [&'static M::DataStruct],
    ) -> Self {
        Self { trie, values }
    }
}

impl<M: DataMarker> super::private::Sealed for DataRef<M> {}
impl<M: DataMarker> super::DataStore<M> for DataRef<M> {
    fn get(
        &self,
        id: DataIdentifierBorrowed,
        attributes_prefix_match: bool,
    ) -> Option<DataPayload<M>> {
        get_index(self.trie, id, attributes_prefix_match)
            // Safety: Allowed since `i` came from the trie and the field safety invariant
            .map(|i| unsafe { self.values.get_unchecked(i) })
            .copied()
            .map(DataPayload::from_static_ref)
    }

    #[cfg(feature = "alloc")]
    type IterReturn = core::iter::FilterMap<
        zerotrie::ZeroTrieStringIterator<'static>,
        fn((alloc::string::String, usize)) -> Option<DataIdentifierCow<'static>>,
    >;
    #[cfg(feature = "alloc")]
    fn iter(&'static self) -> Self::IterReturn {
        iter(&self.trie)
    }
}

/// Optimized data stored as a single VarZeroSlice to reduce token count
#[allow(missing_debug_implementations)] // Debug on this will not be too useful
pub struct DataForVarULEs<M: DataMarker>
where
    M::DataStruct: MaybeAsVarULE,
    M::DataStruct: ZeroFrom<'static, <M::DataStruct as MaybeAsVarULE>::EncodedStruct>,
{
    // Unsafe invariant: actual values contained MUST be valid indices into `values`
    trie: ZeroTrieSimpleAscii<&'static [u8]>,
    values: &'static VarZeroSlice<<M::DataStruct as MaybeAsVarULE>::EncodedStruct>,
}

impl<M: DataMarker> super::private::Sealed for DataForVarULEs<M>
where
    M::DataStruct: MaybeAsVarULE,
    M::DataStruct: ZeroFrom<'static, <M::DataStruct as MaybeAsVarULE>::EncodedStruct>,
{
}

impl<M: DataMarker> DataForVarULEs<M>
where
    M::DataStruct: MaybeAsVarULE,
    M::DataStruct: ZeroFrom<'static, <M::DataStruct as MaybeAsVarULE>::EncodedStruct>,
{
    /// Construct from a trie and values
    ///
    /// # Safety
    /// The actual values contained in the trie must be valid indices into `values`
    pub const unsafe fn from_trie_and_values_unchecked(
        trie: ZeroTrieSimpleAscii<&'static [u8]>,
        values: &'static VarZeroSlice<<M::DataStruct as MaybeAsVarULE>::EncodedStruct>,
    ) -> Self {
        Self { trie, values }
    }
}

impl<M: DataMarker> super::DataStore<M> for DataForVarULEs<M>
where
    M::DataStruct: MaybeAsVarULE,
    M::DataStruct: ZeroFrom<'static, <M::DataStruct as MaybeAsVarULE>::EncodedStruct>,
{
    fn get(
        &self,
        id: DataIdentifierBorrowed,
        attributes_prefix_match: bool,
    ) -> Option<DataPayload<M>> {
        get_index(self.trie, id, attributes_prefix_match)
            // Safety: Allowed since `i` came from the trie and the field safety invariant
            .map(|i| unsafe { self.values.get_unchecked(i) })
            .map(M::DataStruct::zero_from)
            .map(DataPayload::from_owned)
    }

    #[cfg(feature = "alloc")]
    type IterReturn = core::iter::FilterMap<
        zerotrie::ZeroTrieStringIterator<'static>,
        fn((alloc::string::String, usize)) -> Option<DataIdentifierCow<'static>>,
    >;
    #[cfg(feature = "alloc")]
    fn iter(&'static self) -> Self::IterReturn {
        iter(&self.trie)
    }
}
