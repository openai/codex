use core::fmt;
use std::{collections::BTreeMap, num::NonZeroU64};

use crate::{
    decoder::ifd::Entry,
    tags::{IfdPointer, Tag},
};

/// An Image File Directory (IFD).
///
/// A directory is a map of [`Tag`]s to [`Value`](crate::decoder::ifd::Value)s. The values are
/// stored anywhere in the file, with the directory containing the offsets and length of the
/// associated values for each tag present in the directory.
///
/// A directory can be created with with
/// [`Decoder::read_directory`](crate::decoder::Decoder::read_directory) or as an empty directory
/// to be extended with entries. A directory may be used with
/// [`Decoder::read_directory_tags`](crate::decoder::Decoder::read_directory_tags) to read the
/// values associated with tags from an underlying file.
#[doc(alias = "IFD")]
pub struct Directory {
    /// There are at most `u16::MAX` entries in any single directory, the count is stored as a
    /// 2-byte value. The order in the file is implied to be ascending by tag value (the decoder
    /// does not mind unordered entries).
    pub(crate) entries: BTreeMap<u16, Entry>,
    pub(crate) next_ifd: Option<NonZeroU64>,
}

impl Directory {
    /// Create a directory in an initial state without entries. Note that an empty directory can
    /// not be encoded in a file, it must contain at least one entry.
    pub fn empty() -> Self {
        Directory {
            entries: BTreeMap::new(),
            next_ifd: None,
        }
    }

    /// Retrieve the value associated with a tag.
    pub fn get(&self, tag: Tag) -> Option<&Entry> {
        self.entries.get(&tag.to_u16())
    }

    /// Check if the directory contains a specified tag.
    pub fn contains(&self, tag: Tag) -> bool {
        self.entries.contains_key(&tag.to_u16())
    }

    /// Iterate over all known and unknown tags in this directory.
    pub fn iter(&self) -> impl Iterator<Item = (Tag, &Entry)> + '_ {
        self.entries
            .iter()
            .map(|(k, v)| (Tag::from_u16_exhaustive(*k), v))
    }

    /// Insert additional entries into the directory.
    ///
    /// Note that a directory can contain at most `u16::MAX` values. There may be one entry that
    /// does not fit into the directory. This entry is silently ignored (please check [`Self::len`]
    /// to detect the condition). Providing a tag multiple times or a tag that already exists
    /// within this directory overwrites the entry.
    pub fn extend(&mut self, iter: impl IntoIterator<Item = (Tag, Entry)>) {
        // Code size conscious extension, avoid monomorphic extensions with the assumption of these
        // not being performance sensitive in practice. (Maybe we have a polymorphic interface for
        // the crate usage in the future.
        self.extend_inner(iter.into_iter().by_ref())
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        // The keys are `u16`. Since IFDs are required to have at least one entry this would have
        // been a trivial thing to do in the specification by storing it minus one but alas. In
        // BigTIFF the count is stored as 8-bit anyways.
        self.entries.len()
    }

    /// Check if there are any entries in this directory. Note that an empty directory can not be
    /// encoded in the file, it must contain at least one entry.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the pointer to the next IFD, if it was defined.
    pub fn next(&self) -> Option<IfdPointer> {
        self.next_ifd.map(|n| IfdPointer(n.get()))
    }

    pub fn set_next(&mut self, next: Option<IfdPointer>) {
        self.next_ifd = next.and_then(|n| NonZeroU64::new(n.0));
    }

    fn extend_inner(&mut self, iter: &mut dyn Iterator<Item = (Tag, Entry)>) {
        for (tag, entry) in iter {
            // If the tag is already present, it will be overwritten.
            let map_entry = self.entries.entry(tag.to_u16());

            match map_entry {
                std::collections::btree_map::Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(entry);
                }
                std::collections::btree_map::Entry::Occupied(mut occupied_entry) => {
                    occupied_entry.insert(entry);
                }
            }
        }
    }
}

impl fmt::Debug for Directory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Directory")
            .field(
                "entries",
                &self.entries.iter().map(|(k, v)| (Tag::from_u16(*k), v)),
            )
            .field("next_ifd", &self.next_ifd)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::Directory;
    use crate::{decoder::ifd::Entry, tags::Tag};

    #[test]
    fn directory_multiple_entries() {
        let mut dir = Directory::empty();
        assert_eq!(dir.len(), 0);

        dir.extend((0..=u16::MAX).map(|i| {
            let tag = Tag::Unknown(1);
            let entry = Entry::new_u64(crate::tags::Type::BYTE, i.into(), [0; 8]);
            (tag, entry)
        }));

        assert_eq!(dir.len(), 1, "Only one tag was ever modified");

        assert_eq!(
            dir.get(Tag::Unknown(1))
                .expect("tag 1 should be present after this chain")
                .count(),
            u16::MAX.into()
        );
    }

    #[test]
    fn iteration_order() {
        let mut dir = Directory::empty();
        assert_eq!(dir.len(), 0);

        let fake_entry = Entry::new_u64(crate::tags::Type::BYTE, 0, [0; 8]);
        dir.extend((0..32).map(|i| {
            let tag = Tag::Unknown(i);
            let entry = fake_entry.clone();
            (tag, entry)
        }));

        let iter_order: Vec<u16> = dir.iter().map(|(tag, _e)| tag.to_u16()).collect();
        assert_eq!(
            iter_order,
            (0..32).collect::<Vec<_>>(),
            "Tags must be in ascending order according to the specification"
        );
    }
}
