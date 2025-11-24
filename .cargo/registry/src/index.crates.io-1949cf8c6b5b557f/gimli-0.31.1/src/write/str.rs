use alloc::vec::Vec;
use indexmap::IndexSet;
use std::ops::{Deref, DerefMut};

use crate::common::{DebugLineStrOffset, DebugStrOffset, SectionId};
use crate::write::{BaseId, Result, Section, Writer};

// Requirements:
// - values are `[u8]`, null bytes are not allowed
// - insertion returns a fixed id
// - inserting a duplicate returns the id of the existing value
// - able to convert an id to a section offset
// Optional?
// - able to get an existing value given an id
//
// Limitations of current implementation (using IndexSet):
// - inserting requires either an allocation for duplicates,
//   or a double lookup for non-duplicates
// - doesn't preserve offsets when updating an existing `.debug_str` section
//
// Possible changes:
// - calculate offsets as we add values, and use that as the id.
//   This would avoid the need for DebugStrOffsets but would make it
//   hard to implement `get`.
macro_rules! define_string_table {
    ($name:ident, $id:ident, $section:ident, $offsets:ident, $docs:expr) => {
        #[doc=$docs]
        #[derive(Debug, Default)]
        pub struct $name {
            base_id: BaseId,
            strings: IndexSet<Vec<u8>>,
        }

        impl $name {
            /// Add a string to the string table and return its id.
            ///
            /// If the string already exists, then return the id of the existing string.
            ///
            /// # Panics
            ///
            /// Panics if `bytes` contains a null byte.
            pub fn add<T>(&mut self, bytes: T) -> $id
            where
                T: Into<Vec<u8>>,
            {
                let bytes = bytes.into();
                assert!(!bytes.contains(&0));
                let (index, _) = self.strings.insert_full(bytes);
                $id::new(self.base_id, index)
            }

            /// Return the number of strings in the table.
            #[inline]
            pub fn count(&self) -> usize {
                self.strings.len()
            }

            /// Get a reference to a string in the table.
            ///
            /// # Panics
            ///
            /// Panics if `id` is invalid.
            pub fn get(&self, id: $id) -> &[u8] {
                debug_assert_eq!(self.base_id, id.base_id);
                self.strings.get_index(id.index).map(Vec::as_slice).unwrap()
            }

            /// Write the string table to the `.debug_str` section.
            ///
            /// Returns the offsets at which the strings are written.
            pub fn write<W: Writer>(&self, w: &mut $section<W>) -> Result<$offsets> {
                let mut offsets = Vec::new();
                for bytes in self.strings.iter() {
                    offsets.push(w.offset());
                    w.write(bytes)?;
                    w.write_u8(0)?;
                }

                Ok($offsets {
                    base_id: self.base_id,
                    offsets,
                })
            }
        }
    };
}

define_id!(StringId, "An identifier for a string in a `StringTable`.");

define_string_table!(
    StringTable,
    StringId,
    DebugStr,
    DebugStrOffsets,
    "A table of strings that will be stored in a `.debug_str` section."
);

define_section!(DebugStr, DebugStrOffset, "A writable `.debug_str` section.");

define_offsets!(
    DebugStrOffsets: StringId => DebugStrOffset,
    "The section offsets of all strings within a `.debug_str` section."
);

define_id!(
    LineStringId,
    "An identifier for a string in a `LineStringTable`."
);

define_string_table!(
    LineStringTable,
    LineStringId,
    DebugLineStr,
    DebugLineStrOffsets,
    "A table of strings that will be stored in a `.debug_line_str` section."
);

define_section!(
    DebugLineStr,
    DebugLineStrOffset,
    "A writable `.debug_line_str` section."
);

define_offsets!(
    DebugLineStrOffsets: LineStringId => DebugLineStrOffset,
    "The section offsets of all strings within a `.debug_line_str` section."
);

#[cfg(test)]
#[cfg(feature = "read")]
mod tests {
    use super::*;
    use crate::read;
    use crate::write::EndianVec;
    use crate::LittleEndian;

    #[test]
    fn test_string_table() {
        let mut strings = StringTable::default();
        assert_eq!(strings.count(), 0);
        let id1 = strings.add(&b"one"[..]);
        let id2 = strings.add(&b"two"[..]);
        assert_eq!(strings.add(&b"one"[..]), id1);
        assert_eq!(strings.add(&b"two"[..]), id2);
        assert_eq!(strings.get(id1), &b"one"[..]);
        assert_eq!(strings.get(id2), &b"two"[..]);
        assert_eq!(strings.count(), 2);

        let mut debug_str = DebugStr::from(EndianVec::new(LittleEndian));
        let offsets = strings.write(&mut debug_str).unwrap();
        assert_eq!(debug_str.slice(), b"one\0two\0");
        assert_eq!(offsets.get(id1), DebugStrOffset(0));
        assert_eq!(offsets.get(id2), DebugStrOffset(4));
        assert_eq!(offsets.count(), 2);
    }

    #[test]
    fn test_string_table_read() {
        let mut strings = StringTable::default();
        let id1 = strings.add(&b"one"[..]);
        let id2 = strings.add(&b"two"[..]);

        let mut debug_str = DebugStr::from(EndianVec::new(LittleEndian));
        let offsets = strings.write(&mut debug_str).unwrap();

        let read_debug_str = read::DebugStr::new(debug_str.slice(), LittleEndian);
        let str1 = read_debug_str.get_str(offsets.get(id1)).unwrap();
        let str2 = read_debug_str.get_str(offsets.get(id2)).unwrap();
        assert_eq!(str1.slice(), &b"one"[..]);
        assert_eq!(str2.slice(), &b"two"[..]);
    }
}
