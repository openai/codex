use alloc::vec::Vec;

#[cfg(feature = "write_std")]
type IndexSet<K> = indexmap::IndexSet<K>;
#[cfg(not(feature = "write_std"))]
type IndexSet<K> = indexmap::IndexSet<K, hashbrown::DefaultHashBuilder>;

/// An identifier for an entry in a string table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringId(usize);

#[derive(Debug, Default)]
pub(crate) struct StringTable<'a> {
    strings: IndexSet<&'a [u8]>,
    offsets: Vec<usize>,
}

impl<'a> StringTable<'a> {
    /// Add a string to the string table.
    ///
    /// Panics if the string table has already been written, or
    /// if the string contains a null byte.
    pub fn add(&mut self, string: &'a [u8]) -> StringId {
        assert!(self.offsets.is_empty());
        assert!(!string.contains(&0));
        let id = self.strings.insert_full(string).0;
        StringId(id)
    }

    /// Return the id of the given string.
    ///
    /// Panics if the string is not in the string table.
    #[allow(dead_code)]
    pub fn get_id(&self, string: &[u8]) -> StringId {
        let id = self.strings.get_index_of(string).unwrap();
        StringId(id)
    }

    /// Return the string for the given id.
    ///
    /// Panics if the string is not in the string table.
    #[allow(dead_code)]
    pub fn get_string(&self, id: StringId) -> &'a [u8] {
        self.strings.get_index(id.0).unwrap()
    }

    /// Return the offset of the given string.
    ///
    /// Panics if the string table has not been written, or
    /// if the string is not in the string table.
    pub fn get_offset(&self, id: StringId) -> usize {
        self.offsets[id.0]
    }

    /// Append the string table to the given `Vec`, and
    /// calculate the list of string offsets.
    ///
    /// `base` is the initial string table offset. For example,
    /// this should be 1 for ELF, to account for the initial
    /// null byte (which must have been written by the caller).
    ///
    /// Panics if the string table has already been written.
    pub fn write(&mut self, base: usize, w: &mut Vec<u8>) {
        assert!(self.offsets.is_empty());

        let mut ids: Vec<_> = (0..self.strings.len()).collect();
        sort(&mut ids, 1, &self.strings);

        self.offsets = vec![0; ids.len()];
        let mut offset = base;
        let mut previous = &[][..];
        for id in ids {
            let string = self.strings.get_index(id).unwrap();
            if previous.ends_with(string) {
                self.offsets[id] = offset - string.len() - 1;
            } else {
                self.offsets[id] = offset;
                w.extend_from_slice(string);
                w.push(0);
                offset += string.len() + 1;
                previous = string;
            }
        }
    }

    /// Calculate the size in bytes of the string table.
    ///
    /// `base` is the initial string table offset. For example,
    /// this should be 1 for ELF, to account for the initial
    /// null byte.
    #[allow(dead_code)]
    pub fn size(&self, base: usize) -> usize {
        // TODO: cache this result?
        let mut ids: Vec<_> = (0..self.strings.len()).collect();
        sort(&mut ids, 1, &self.strings);

        let mut size = base;
        let mut previous = &[][..];
        for id in ids {
            let string = self.strings.get_index(id).unwrap();
            if !previous.ends_with(string) {
                size += string.len() + 1;
                previous = string;
            }
        }
        size
    }
}

// Multi-key quicksort.
//
// Ordering is such that if a string is a suffix of at least one other string,
// then it is placed immediately after one of those strings. That is:
// - comparison starts at the end of the string
// - shorter strings come later
//
// Based on the implementation in LLVM.
fn sort(mut ids: &mut [usize], mut pos: usize, strings: &IndexSet<&[u8]>) {
    loop {
        if ids.len() <= 1 {
            return;
        }

        let pivot = byte(ids[0], pos, strings);
        let mut lower = 0;
        let mut upper = ids.len();
        let mut i = 1;
        while i < upper {
            let b = byte(ids[i], pos, strings);
            if b > pivot {
                ids.swap(lower, i);
                lower += 1;
                i += 1;
            } else if b < pivot {
                upper -= 1;
                ids.swap(upper, i);
            } else {
                i += 1;
            }
        }

        sort(&mut ids[..lower], pos, strings);
        sort(&mut ids[upper..], pos, strings);

        if pivot == 0 {
            return;
        }
        ids = &mut ids[lower..upper];
        pos += 1;
    }
}

fn byte(id: usize, pos: usize, strings: &IndexSet<&[u8]>) -> u8 {
    let string = strings.get_index(id).unwrap();
    let len = string.len();
    if len >= pos {
        string[len - pos]
    } else {
        // We know the strings don't contain null bytes.
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_table() {
        let mut table = StringTable::default();
        let id0 = table.add(b"");
        let id1 = table.add(b"foo");
        let id2 = table.add(b"bar");
        let id3 = table.add(b"foobar");

        let mut data = Vec::new();
        data.push(0);
        table.write(1, &mut data);
        assert_eq!(data, b"\0foobar\0foo\0");

        assert_eq!(table.get_offset(id0), 11);
        assert_eq!(table.get_offset(id1), 8);
        assert_eq!(table.get_offset(id2), 4);
        assert_eq!(table.get_offset(id3), 1);
    }
}
