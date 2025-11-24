use crate::util::int::Usize;

/// A representation of byte oriented equivalence classes.
///
/// This is used in finite state machines to reduce the size of the transition
/// table. This can have a particularly large impact not only on the total size
/// of an FSM, but also on FSM build times because it reduces the number of
/// transitions that need to be visited/set.
#[derive(Clone, Copy)]
pub(crate) struct ByteClasses([u8; 256]);

impl ByteClasses {
    /// Creates a new set of equivalence classes where all bytes are mapped to
    /// the same class.
    pub(crate) fn empty() -> ByteClasses {
        ByteClasses([0; 256])
    }

    /// Creates a new set of equivalence classes where each byte belongs to
    /// its own equivalence class.
    pub(crate) fn singletons() -> ByteClasses {
        let mut classes = ByteClasses::empty();
        for b in 0..=255 {
            classes.set(b, b);
        }
        classes
    }

    /// Set the equivalence class for the given byte.
    #[inline]
    pub(crate) fn set(&mut self, byte: u8, class: u8) {
        self.0[usize::from(byte)] = class;
    }

    /// Get the equivalence class for the given byte.
    #[inline]
    pub(crate) fn get(&self, byte: u8) -> u8 {
        self.0[usize::from(byte)]
    }

    /// Return the total number of elements in the alphabet represented by
    /// these equivalence classes. Equivalently, this returns the total number
    /// of equivalence classes.
    #[inline]
    pub(crate) fn alphabet_len(&self) -> usize {
        // Add one since the number of equivalence classes is one bigger than
        // the last one.
        usize::from(self.0[255]) + 1
    }

    /// Returns the stride, as a base-2 exponent, required for these
    /// equivalence classes.
    ///
    /// The stride is always the smallest power of 2 that is greater than or
    /// equal to the alphabet length. This is done so that converting between
    /// state IDs and indices can be done with shifts alone, which is much
    /// faster than integer division. The "stride2" is the exponent. i.e.,
    /// `2^stride2 = stride`.
    pub(crate) fn stride2(&self) -> usize {
        let zeros = self.alphabet_len().next_power_of_two().trailing_zeros();
        usize::try_from(zeros).unwrap()
    }

    /// Returns the stride for these equivalence classes, which corresponds
    /// to the smallest power of 2 greater than or equal to the number of
    /// equivalence classes.
    pub(crate) fn stride(&self) -> usize {
        1 << self.stride2()
    }

    /// Returns true if and only if every byte in this class maps to its own
    /// equivalence class. Equivalently, there are 257 equivalence classes
    /// and each class contains exactly one byte (plus the special EOI class).
    #[inline]
    pub(crate) fn is_singleton(&self) -> bool {
        self.alphabet_len() == 256
    }

    /// Returns an iterator over all equivalence classes in this set.
    pub(crate) fn iter(&self) -> ByteClassIter {
        ByteClassIter { it: 0..self.alphabet_len() }
    }

    /// Returns an iterator of the bytes in the given equivalence class.
    pub(crate) fn elements(&self, class: u8) -> ByteClassElements {
        ByteClassElements { classes: self, class, bytes: 0..=255 }
    }

    /// Returns an iterator of byte ranges in the given equivalence class.
    ///
    /// That is, a sequence of contiguous ranges are returned. Typically, every
    /// class maps to a single contiguous range.
    fn element_ranges(&self, class: u8) -> ByteClassElementRanges {
        ByteClassElementRanges { elements: self.elements(class), range: None }
    }
}

impl core::fmt::Debug for ByteClasses {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_singleton() {
            write!(f, "ByteClasses(<one-class-per-byte>)")
        } else {
            write!(f, "ByteClasses(")?;
            for (i, class) in self.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{:?} => [", class)?;
                for (start, end) in self.element_ranges(class) {
                    if start == end {
                        write!(f, "{:?}", start)?;
                    } else {
                        write!(f, "{:?}-{:?}", start, end)?;
                    }
                }
                write!(f, "]")?;
            }
            write!(f, ")")
        }
    }
}

/// An iterator over each equivalence class.
#[derive(Debug)]
pub(crate) struct ByteClassIter {
    it: core::ops::Range<usize>,
}

impl Iterator for ByteClassIter {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        self.it.next().map(|class| class.as_u8())
    }
}

/// An iterator over all elements in a specific equivalence class.
#[derive(Debug)]
pub(crate) struct ByteClassElements<'a> {
    classes: &'a ByteClasses,
    class: u8,
    bytes: core::ops::RangeInclusive<u8>,
}

impl<'a> Iterator for ByteClassElements<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        while let Some(byte) = self.bytes.next() {
            if self.class == self.classes.get(byte) {
                return Some(byte);
            }
        }
        None
    }
}

/// An iterator over all elements in an equivalence class expressed as a
/// sequence of contiguous ranges.
#[derive(Debug)]
pub(crate) struct ByteClassElementRanges<'a> {
    elements: ByteClassElements<'a>,
    range: Option<(u8, u8)>,
}

impl<'a> Iterator for ByteClassElementRanges<'a> {
    type Item = (u8, u8);

    fn next(&mut self) -> Option<(u8, u8)> {
        loop {
            let element = match self.elements.next() {
                None => return self.range.take(),
                Some(element) => element,
            };
            match self.range.take() {
                None => {
                    self.range = Some((element, element));
                }
                Some((start, end)) => {
                    if usize::from(end) + 1 != usize::from(element) {
                        self.range = Some((element, element));
                        return Some((start, end));
                    }
                    self.range = Some((start, element));
                }
            }
        }
    }
}

/// A partitioning of bytes into equivalence classes.
///
/// A byte class set keeps track of an *approximation* of equivalence classes
/// of bytes during NFA construction. That is, every byte in an equivalence
/// class cannot discriminate between a match and a non-match.
///
/// Note that this may not compute the minimal set of equivalence classes.
/// Basically, any byte in a pattern given to the noncontiguous NFA builder
/// will automatically be treated as its own equivalence class. All other
/// bytes---any byte not in any pattern---will be treated as their own
/// equivalence classes. In theory, all bytes not in any pattern should
/// be part of a single equivalence class, but in practice, we only treat
/// contiguous ranges of bytes as an equivalence class. So the number of
/// classes computed may be bigger than necessary. This usually doesn't make
/// much of a difference, and keeps the implementation simple.
#[derive(Clone, Debug)]
pub(crate) struct ByteClassSet(ByteSet);

impl Default for ByteClassSet {
    fn default() -> ByteClassSet {
        ByteClassSet::empty()
    }
}

impl ByteClassSet {
    /// Create a new set of byte classes where all bytes are part of the same
    /// equivalence class.
    pub(crate) fn empty() -> Self {
        ByteClassSet(ByteSet::empty())
    }

    /// Indicate the the range of byte given (inclusive) can discriminate a
    /// match between it and all other bytes outside of the range.
    pub(crate) fn set_range(&mut self, start: u8, end: u8) {
        debug_assert!(start <= end);
        if start > 0 {
            self.0.add(start - 1);
        }
        self.0.add(end);
    }

    /// Convert this boolean set to a map that maps all byte values to their
    /// corresponding equivalence class. The last mapping indicates the largest
    /// equivalence class identifier (which is never bigger than 255).
    pub(crate) fn byte_classes(&self) -> ByteClasses {
        let mut classes = ByteClasses::empty();
        let mut class = 0u8;
        let mut b = 0u8;
        loop {
            classes.set(b, class);
            if b == 255 {
                break;
            }
            if self.0.contains(b) {
                class = class.checked_add(1).unwrap();
            }
            b = b.checked_add(1).unwrap();
        }
        classes
    }
}

/// A simple set of bytes that is reasonably cheap to copy and allocation free.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ByteSet {
    bits: BitSet,
}

/// The representation of a byte set. Split out so that we can define a
/// convenient Debug impl for it while keeping "ByteSet" in the output.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct BitSet([u128; 2]);

impl ByteSet {
    /// Create an empty set of bytes.
    pub(crate) fn empty() -> ByteSet {
        ByteSet { bits: BitSet([0; 2]) }
    }

    /// Add a byte to this set.
    ///
    /// If the given byte already belongs to this set, then this is a no-op.
    pub(crate) fn add(&mut self, byte: u8) {
        let bucket = byte / 128;
        let bit = byte % 128;
        self.bits.0[usize::from(bucket)] |= 1 << bit;
    }

    /// Return true if and only if the given byte is in this set.
    pub(crate) fn contains(&self, byte: u8) -> bool {
        let bucket = byte / 128;
        let bit = byte % 128;
        self.bits.0[usize::from(bucket)] & (1 << bit) > 0
    }
}

impl core::fmt::Debug for BitSet {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut fmtd = f.debug_set();
        for b in 0u8..=255 {
            if (ByteSet { bits: *self }).contains(b) {
                fmtd.entry(&b);
            }
        }
        fmtd.finish()
    }
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use super::*;

    #[test]
    fn byte_classes() {
        let mut set = ByteClassSet::empty();
        set.set_range(b'a', b'z');

        let classes = set.byte_classes();
        assert_eq!(classes.get(0), 0);
        assert_eq!(classes.get(1), 0);
        assert_eq!(classes.get(2), 0);
        assert_eq!(classes.get(b'a' - 1), 0);
        assert_eq!(classes.get(b'a'), 1);
        assert_eq!(classes.get(b'm'), 1);
        assert_eq!(classes.get(b'z'), 1);
        assert_eq!(classes.get(b'z' + 1), 2);
        assert_eq!(classes.get(254), 2);
        assert_eq!(classes.get(255), 2);

        let mut set = ByteClassSet::empty();
        set.set_range(0, 2);
        set.set_range(4, 6);
        let classes = set.byte_classes();
        assert_eq!(classes.get(0), 0);
        assert_eq!(classes.get(1), 0);
        assert_eq!(classes.get(2), 0);
        assert_eq!(classes.get(3), 1);
        assert_eq!(classes.get(4), 2);
        assert_eq!(classes.get(5), 2);
        assert_eq!(classes.get(6), 2);
        assert_eq!(classes.get(7), 3);
        assert_eq!(classes.get(255), 3);
    }

    #[test]
    fn full_byte_classes() {
        let mut set = ByteClassSet::empty();
        for b in 0u8..=255 {
            set.set_range(b, b);
        }
        assert_eq!(set.byte_classes().alphabet_len(), 256);
    }

    #[test]
    fn elements_typical() {
        let mut set = ByteClassSet::empty();
        set.set_range(b'b', b'd');
        set.set_range(b'g', b'm');
        set.set_range(b'z', b'z');
        let classes = set.byte_classes();
        // class 0: \x00-a
        // class 1: b-d
        // class 2: e-f
        // class 3: g-m
        // class 4: n-y
        // class 5: z-z
        // class 6: \x7B-\xFF
        assert_eq!(classes.alphabet_len(), 7);

        let elements = classes.elements(0).collect::<Vec<_>>();
        assert_eq!(elements.len(), 98);
        assert_eq!(elements[0], b'\x00');
        assert_eq!(elements[97], b'a');

        let elements = classes.elements(1).collect::<Vec<_>>();
        assert_eq!(elements, vec![b'b', b'c', b'd'],);

        let elements = classes.elements(2).collect::<Vec<_>>();
        assert_eq!(elements, vec![b'e', b'f'],);

        let elements = classes.elements(3).collect::<Vec<_>>();
        assert_eq!(elements, vec![b'g', b'h', b'i', b'j', b'k', b'l', b'm',],);

        let elements = classes.elements(4).collect::<Vec<_>>();
        assert_eq!(elements.len(), 12);
        assert_eq!(elements[0], b'n');
        assert_eq!(elements[11], b'y');

        let elements = classes.elements(5).collect::<Vec<_>>();
        assert_eq!(elements, vec![b'z']);

        let elements = classes.elements(6).collect::<Vec<_>>();
        assert_eq!(elements.len(), 133);
        assert_eq!(elements[0], b'\x7B');
        assert_eq!(elements[132], b'\xFF');
    }

    #[test]
    fn elements_singletons() {
        let classes = ByteClasses::singletons();
        assert_eq!(classes.alphabet_len(), 256);

        let elements = classes.elements(b'a').collect::<Vec<_>>();
        assert_eq!(elements, vec![b'a']);
    }

    #[test]
    fn elements_empty() {
        let classes = ByteClasses::empty();
        assert_eq!(classes.alphabet_len(), 1);

        let elements = classes.elements(0).collect::<Vec<_>>();
        assert_eq!(elements.len(), 256);
        assert_eq!(elements[0], b'\x00');
        assert_eq!(elements[255], b'\xFF');
    }
}
