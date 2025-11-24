/*!
This module provides APIs for dealing with the alphabets of finite state
machines.

There are two principal types in this module, [`ByteClasses`] and [`Unit`].
The former defines the alphabet of a finite state machine while the latter
represents an element of that alphabet.

To a first approximation, the alphabet of all automata in this crate is just
a `u8`. Namely, every distinct byte value. All 256 of them. In practice, this
can be quite wasteful when building a transition table for a DFA, since it
requires storing a state identifier for each element in the alphabet. Instead,
we collapse the alphabet of an automaton down into equivalence classes, where
every byte in the same equivalence class never discriminates between a match or
a non-match from any other byte in the same class. For example, in the regex
`[a-z]+`, then you could consider it having an alphabet consisting of two
equivalence classes: `a-z` and everything else. In terms of the transitions on
an automaton, it doesn't actually require representing every distinct byte.
Just the equivalence classes.

The downside of equivalence classes is that, of course, searching a haystack
deals with individual byte values. Those byte values need to be mapped to
their corresponding equivalence class. This is what `ByteClasses` does. In
practice, doing this for every state transition has negligible impact on modern
CPUs. Moreover, it helps make more efficient use of the CPU cache by (possibly
considerably) shrinking the size of the transition table.

One last hiccup concerns `Unit`. Namely, because of look-around and how the
DFAs in this crate work, we need to add a sentinel value to our alphabet
of equivalence classes that represents the "end" of a search. We call that
sentinel [`Unit::eoi`] or "end of input." Thus, a `Unit` is either an
equivalence class corresponding to a set of bytes, or it is a special "end of
input" sentinel.

In general, you should not expect to need either of these types unless you're
doing lower level shenanigans with DFAs, or even building your own DFAs.
(Although, you don't have to use these types to build your own DFAs of course.)
For example, if you're walking a DFA's state graph, it's probably useful to
make use of [`ByteClasses`] to visit each element in the DFA's alphabet instead
of just visiting every distinct `u8` value. The latter isn't necessarily wrong,
but it could be potentially very wasteful.
*/
use crate::util::{
    escape::DebugByte,
    wire::{self, DeserializeError, SerializeError},
};

/// Unit represents a single unit of haystack for DFA based regex engines.
///
/// It is not expected for consumers of this crate to need to use this type
/// unless they are implementing their own DFA. And even then, it's not
/// required: implementors may use other techniques to handle haystack units.
///
/// Typically, a single unit of haystack for a DFA would be a single byte.
/// However, for the DFAs in this crate, matches are delayed by a single byte
/// in order to handle look-ahead assertions (`\b`, `$` and `\z`). Thus, once
/// we have consumed the haystack, we must run the DFA through one additional
/// transition using a unit that indicates the haystack has ended.
///
/// There is no way to represent a sentinel with a `u8` since all possible
/// values *may* be valid haystack units to a DFA, therefore this type
/// explicitly adds room for a sentinel value.
///
/// The sentinel EOI value is always its own equivalence class and is
/// ultimately represented by adding 1 to the maximum equivalence class value.
/// So for example, the regex `^[a-z]+$` might be split into the following
/// equivalence classes:
///
/// ```text
/// 0 => [\x00-`]
/// 1 => [a-z]
/// 2 => [{-\xFF]
/// 3 => [EOI]
/// ```
///
/// Where EOI is the special sentinel value that is always in its own
/// singleton equivalence class.
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub struct Unit(UnitKind);

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
enum UnitKind {
    /// Represents a byte value, or more typically, an equivalence class
    /// represented as a byte value.
    U8(u8),
    /// Represents the "end of input" sentinel. We regretably use a `u16`
    /// here since the maximum sentinel value is `256`. Thankfully, we don't
    /// actually store a `Unit` anywhere, so this extra space shouldn't be too
    /// bad.
    EOI(u16),
}

impl Unit {
    /// Create a new haystack unit from a byte value.
    ///
    /// All possible byte values are legal. However, when creating a haystack
    /// unit for a specific DFA, one should be careful to only construct units
    /// that are in that DFA's alphabet. Namely, one way to compact a DFA's
    /// in-memory representation is to collapse its transitions to a set of
    /// equivalence classes into a set of all possible byte values. If a DFA
    /// uses equivalence classes instead of byte values, then the byte given
    /// here should be the equivalence class.
    pub fn u8(byte: u8) -> Unit {
        Unit(UnitKind::U8(byte))
    }

    /// Create a new "end of input" haystack unit.
    ///
    /// The value given is the sentinel value used by this unit to represent
    /// the "end of input." The value should be the total number of equivalence
    /// classes in the corresponding alphabet. Its maximum value is `256`,
    /// which occurs when every byte is its own equivalence class.
    ///
    /// # Panics
    ///
    /// This panics when `num_byte_equiv_classes` is greater than `256`.
    pub fn eoi(num_byte_equiv_classes: usize) -> Unit {
        assert!(
            num_byte_equiv_classes <= 256,
            "max number of byte-based equivalent classes is 256, but got {}",
            num_byte_equiv_classes,
        );
        Unit(UnitKind::EOI(u16::try_from(num_byte_equiv_classes).unwrap()))
    }

    /// If this unit is not an "end of input" sentinel, then returns its
    /// underlying byte value. Otherwise return `None`.
    pub fn as_u8(self) -> Option<u8> {
        match self.0 {
            UnitKind::U8(b) => Some(b),
            UnitKind::EOI(_) => None,
        }
    }

    /// If this unit is an "end of input" sentinel, then return the underlying
    /// sentinel value that was given to [`Unit::eoi`]. Otherwise return
    /// `None`.
    pub fn as_eoi(self) -> Option<u16> {
        match self.0 {
            UnitKind::U8(_) => None,
            UnitKind::EOI(sentinel) => Some(sentinel),
        }
    }

    /// Return this unit as a `usize`, regardless of whether it is a byte value
    /// or an "end of input" sentinel. In the latter case, the underlying
    /// sentinel value given to [`Unit::eoi`] is returned.
    pub fn as_usize(self) -> usize {
        match self.0 {
            UnitKind::U8(b) => usize::from(b),
            UnitKind::EOI(eoi) => usize::from(eoi),
        }
    }

    /// Returns true if and only of this unit is a byte value equivalent to the
    /// byte given. This always returns false when this is an "end of input"
    /// sentinel.
    pub fn is_byte(self, byte: u8) -> bool {
        self.as_u8().map_or(false, |b| b == byte)
    }

    /// Returns true when this unit represents an "end of input" sentinel.
    pub fn is_eoi(self) -> bool {
        self.as_eoi().is_some()
    }

    /// Returns true when this unit corresponds to an ASCII word byte.
    ///
    /// This always returns false when this unit represents an "end of input"
    /// sentinel.
    pub fn is_word_byte(self) -> bool {
        self.as_u8().map_or(false, crate::util::utf8::is_word_byte)
    }
}

impl core::fmt::Debug for Unit {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self.0 {
            UnitKind::U8(b) => write!(f, "{:?}", DebugByte(b)),
            UnitKind::EOI(_) => write!(f, "EOI"),
        }
    }
}

/// A representation of byte oriented equivalence classes.
///
/// This is used in a DFA to reduce the size of the transition table. This can
/// have a particularly large impact not only on the total size of a dense DFA,
/// but also on compile times.
///
/// The essential idea here is that the alphabet of a DFA is shrunk from the
/// usual 256 distinct byte values down to a set of equivalence classes. The
/// guarantee you get is that any byte belonging to the same equivalence class
/// can be treated as if it were any other byte in the same class, and the
/// result of a search wouldn't change.
///
/// # Example
///
/// This example shows how to get byte classes from an
/// [`NFA`](crate::nfa::thompson::NFA) and ask for the class of various bytes.
///
/// ```
/// use regex_automata::nfa::thompson::NFA;
///
/// let nfa = NFA::new("[a-z]+")?;
/// let classes = nfa.byte_classes();
/// // 'a' and 'z' are in the same class for this regex.
/// assert_eq!(classes.get(b'a'), classes.get(b'z'));
/// // But 'a' and 'A' are not.
/// assert_ne!(classes.get(b'a'), classes.get(b'A'));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy)]
pub struct ByteClasses([u8; 256]);

impl ByteClasses {
    /// Creates a new set of equivalence classes where all bytes are mapped to
    /// the same class.
    #[inline]
    pub fn empty() -> ByteClasses {
        ByteClasses([0; 256])
    }

    /// Creates a new set of equivalence classes where each byte belongs to
    /// its own equivalence class.
    #[inline]
    pub fn singletons() -> ByteClasses {
        let mut classes = ByteClasses::empty();
        for b in 0..=255 {
            classes.set(b, b);
        }
        classes
    }

    /// Deserializes a byte class map from the given slice. If the slice is of
    /// insufficient length or otherwise contains an impossible mapping, then
    /// an error is returned. Upon success, the number of bytes read along with
    /// the map are returned. The number of bytes read is always a multiple of
    /// 8.
    pub(crate) fn from_bytes(
        slice: &[u8],
    ) -> Result<(ByteClasses, usize), DeserializeError> {
        wire::check_slice_len(slice, 256, "byte class map")?;
        let mut classes = ByteClasses::empty();
        for (b, &class) in slice[..256].iter().enumerate() {
            classes.set(u8::try_from(b).unwrap(), class);
        }
        // We specifically don't use 'classes.iter()' here because that
        // iterator depends on 'classes.alphabet_len()' being correct. But that
        // is precisely the thing we're trying to verify below!
        for &b in classes.0.iter() {
            if usize::from(b) >= classes.alphabet_len() {
                return Err(DeserializeError::generic(
                    "found equivalence class greater than alphabet len",
                ));
            }
        }
        Ok((classes, 256))
    }

    /// Writes this byte class map to the given byte buffer. if the given
    /// buffer is too small, then an error is returned. Upon success, the total
    /// number of bytes written is returned. The number of bytes written is
    /// guaranteed to be a multiple of 8.
    pub(crate) fn write_to(
        &self,
        mut dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("byte class map"));
        }
        for b in 0..=255 {
            dst[0] = self.get(b);
            dst = &mut dst[1..];
        }
        Ok(nwrite)
    }

    /// Returns the total number of bytes written by `write_to`.
    pub(crate) fn write_to_len(&self) -> usize {
        256
    }

    /// Set the equivalence class for the given byte.
    #[inline]
    pub fn set(&mut self, byte: u8, class: u8) {
        self.0[usize::from(byte)] = class;
    }

    /// Get the equivalence class for the given byte.
    #[inline]
    pub fn get(&self, byte: u8) -> u8 {
        self.0[usize::from(byte)]
    }

    /// Get the equivalence class for the given haystack unit and return the
    /// class as a `usize`.
    #[inline]
    pub fn get_by_unit(&self, unit: Unit) -> usize {
        match unit.0 {
            UnitKind::U8(b) => usize::from(self.get(b)),
            UnitKind::EOI(b) => usize::from(b),
        }
    }

    /// Create a unit that represents the "end of input" sentinel based on the
    /// number of equivalence classes.
    #[inline]
    pub fn eoi(&self) -> Unit {
        // The alphabet length already includes the EOI sentinel, hence why
        // we subtract 1.
        Unit::eoi(self.alphabet_len().checked_sub(1).unwrap())
    }

    /// Return the total number of elements in the alphabet represented by
    /// these equivalence classes. Equivalently, this returns the total number
    /// of equivalence classes.
    #[inline]
    pub fn alphabet_len(&self) -> usize {
        // Add one since the number of equivalence classes is one bigger than
        // the last one. But add another to account for the final EOI class
        // that isn't explicitly represented.
        usize::from(self.0[255]) + 1 + 1
    }

    /// Returns the stride, as a base-2 exponent, required for these
    /// equivalence classes.
    ///
    /// The stride is always the smallest power of 2 that is greater than or
    /// equal to the alphabet length, and the `stride2` returned here is the
    /// exponent applied to `2` to get the smallest power. This is done so that
    /// converting between premultiplied state IDs and indices can be done with
    /// shifts alone, which is much faster than integer division.
    #[inline]
    pub fn stride2(&self) -> usize {
        let zeros = self.alphabet_len().next_power_of_two().trailing_zeros();
        usize::try_from(zeros).unwrap()
    }

    /// Returns true if and only if every byte in this class maps to its own
    /// equivalence class. Equivalently, there are 257 equivalence classes
    /// and each class contains either exactly one byte or corresponds to the
    /// singleton class containing the "end of input" sentinel.
    #[inline]
    pub fn is_singleton(&self) -> bool {
        self.alphabet_len() == 257
    }

    /// Returns an iterator over all equivalence classes in this set.
    #[inline]
    pub fn iter(&self) -> ByteClassIter<'_> {
        ByteClassIter { classes: self, i: 0 }
    }

    /// Returns an iterator over a sequence of representative bytes from each
    /// equivalence class within the range of bytes given.
    ///
    /// When the given range is unbounded on both sides, the iterator yields
    /// exactly N items, where N is equivalent to the number of equivalence
    /// classes. Each item is an arbitrary byte drawn from each equivalence
    /// class.
    ///
    /// This is useful when one is determinizing an NFA and the NFA's alphabet
    /// hasn't been converted to equivalence classes. Picking an arbitrary byte
    /// from each equivalence class then permits a full exploration of the NFA
    /// instead of using every possible byte value and thus potentially saves
    /// quite a lot of redundant work.
    ///
    /// # Example
    ///
    /// This shows an example of what a complete sequence of representatives
    /// might look like from a real example.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, util::alphabet::Unit};
    ///
    /// let nfa = NFA::new("[a-z]+")?;
    /// let classes = nfa.byte_classes();
    /// let reps: Vec<Unit> = classes.representatives(..).collect();
    /// // Note that the specific byte values yielded are not guaranteed!
    /// let expected = vec![
    ///     Unit::u8(b'\x00'),
    ///     Unit::u8(b'a'),
    ///     Unit::u8(b'{'),
    ///     Unit::eoi(3),
    /// ];
    /// assert_eq!(expected, reps);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Note though, that you can ask for an arbitrary range of bytes, and only
    /// representatives for that range will be returned:
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, util::alphabet::Unit};
    ///
    /// let nfa = NFA::new("[a-z]+")?;
    /// let classes = nfa.byte_classes();
    /// let reps: Vec<Unit> = classes.representatives(b'A'..=b'z').collect();
    /// // Note that the specific byte values yielded are not guaranteed!
    /// let expected = vec![
    ///     Unit::u8(b'A'),
    ///     Unit::u8(b'a'),
    /// ];
    /// assert_eq!(expected, reps);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn representatives<R: core::ops::RangeBounds<u8>>(
        &self,
        range: R,
    ) -> ByteClassRepresentatives<'_> {
        use core::ops::Bound;

        let cur_byte = match range.start_bound() {
            Bound::Included(&i) => usize::from(i),
            Bound::Excluded(&i) => usize::from(i).checked_add(1).unwrap(),
            Bound::Unbounded => 0,
        };
        let end_byte = match range.end_bound() {
            Bound::Included(&i) => {
                Some(usize::from(i).checked_add(1).unwrap())
            }
            Bound::Excluded(&i) => Some(usize::from(i)),
            Bound::Unbounded => None,
        };
        assert_ne!(
            cur_byte,
            usize::MAX,
            "start range must be less than usize::MAX",
        );
        ByteClassRepresentatives {
            classes: self,
            cur_byte,
            end_byte,
            last_class: None,
        }
    }

    /// Returns an iterator of the bytes in the given equivalence class.
    ///
    /// This is useful when one needs to know the actual bytes that belong to
    /// an equivalence class. For example, conceptually speaking, accelerating
    /// a DFA state occurs when a state only has a few outgoing transitions.
    /// But in reality, what is required is that there are only a small
    /// number of distinct bytes that can lead to an outgoing transition. The
    /// difference is that any one transition can correspond to an equivalence
    /// class which may contains many bytes. Therefore, DFA state acceleration
    /// considers the actual elements in each equivalence class of each
    /// outgoing transition.
    ///
    /// # Example
    ///
    /// This shows an example of how to get all of the elements in an
    /// equivalence class.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, util::alphabet::Unit};
    ///
    /// let nfa = NFA::new("[a-z]+")?;
    /// let classes = nfa.byte_classes();
    /// let elements: Vec<Unit> = classes.elements(Unit::u8(1)).collect();
    /// let expected: Vec<Unit> = (b'a'..=b'z').map(Unit::u8).collect();
    /// assert_eq!(expected, elements);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn elements(&self, class: Unit) -> ByteClassElements {
        ByteClassElements { classes: self, class, byte: 0 }
    }

    /// Returns an iterator of byte ranges in the given equivalence class.
    ///
    /// That is, a sequence of contiguous ranges are returned. Typically, every
    /// class maps to a single contiguous range.
    fn element_ranges(&self, class: Unit) -> ByteClassElementRanges {
        ByteClassElementRanges { elements: self.elements(class), range: None }
    }
}

impl Default for ByteClasses {
    fn default() -> ByteClasses {
        ByteClasses::singletons()
    }
}

impl core::fmt::Debug for ByteClasses {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_singleton() {
            write!(f, "ByteClasses({{singletons}})")
        } else {
            write!(f, "ByteClasses(")?;
            for (i, class) in self.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{:?} => [", class.as_usize())?;
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
///
/// The last element in this iterator always corresponds to [`Unit::eoi`].
///
/// This is created by the [`ByteClasses::iter`] method.
///
/// The lifetime `'a` refers to the lifetime of the byte classes that this
/// iterator was created from.
#[derive(Debug)]
pub struct ByteClassIter<'a> {
    classes: &'a ByteClasses,
    i: usize,
}

impl<'a> Iterator for ByteClassIter<'a> {
    type Item = Unit;

    fn next(&mut self) -> Option<Unit> {
        if self.i + 1 == self.classes.alphabet_len() {
            self.i += 1;
            Some(self.classes.eoi())
        } else if self.i < self.classes.alphabet_len() {
            let class = u8::try_from(self.i).unwrap();
            self.i += 1;
            Some(Unit::u8(class))
        } else {
            None
        }
    }
}

/// An iterator over representative bytes from each equivalence class.
///
/// This is created by the [`ByteClasses::representatives`] method.
///
/// The lifetime `'a` refers to the lifetime of the byte classes that this
/// iterator was created from.
#[derive(Debug)]
pub struct ByteClassRepresentatives<'a> {
    classes: &'a ByteClasses,
    cur_byte: usize,
    end_byte: Option<usize>,
    last_class: Option<u8>,
}

impl<'a> Iterator for ByteClassRepresentatives<'a> {
    type Item = Unit;

    fn next(&mut self) -> Option<Unit> {
        while self.cur_byte < self.end_byte.unwrap_or(256) {
            let byte = u8::try_from(self.cur_byte).unwrap();
            let class = self.classes.get(byte);
            self.cur_byte += 1;

            if self.last_class != Some(class) {
                self.last_class = Some(class);
                return Some(Unit::u8(byte));
            }
        }
        if self.cur_byte != usize::MAX && self.end_byte.is_none() {
            // Using usize::MAX as a sentinel is OK because we ban usize::MAX
            // from appearing as a start bound in iterator construction. But
            // why do it this way? Well, we want to return the EOI class
            // whenever the end of the given range is unbounded because EOI
            // isn't really a "byte" per se, so the only way it should be
            // excluded is if there is a bounded end to the range. Therefore,
            // when the end is unbounded, we just need to know whether we've
            // reported EOI or not. When we do, we set cur_byte to a value it
            // can never otherwise be.
            self.cur_byte = usize::MAX;
            return Some(self.classes.eoi());
        }
        None
    }
}

/// An iterator over all elements in an equivalence class.
///
/// This is created by the [`ByteClasses::elements`] method.
///
/// The lifetime `'a` refers to the lifetime of the byte classes that this
/// iterator was created from.
#[derive(Debug)]
pub struct ByteClassElements<'a> {
    classes: &'a ByteClasses,
    class: Unit,
    byte: usize,
}

impl<'a> Iterator for ByteClassElements<'a> {
    type Item = Unit;

    fn next(&mut self) -> Option<Unit> {
        while self.byte < 256 {
            let byte = u8::try_from(self.byte).unwrap();
            self.byte += 1;
            if self.class.is_byte(self.classes.get(byte)) {
                return Some(Unit::u8(byte));
            }
        }
        if self.byte < 257 {
            self.byte += 1;
            if self.class.is_eoi() {
                return Some(Unit::eoi(256));
            }
        }
        None
    }
}

/// An iterator over all elements in an equivalence class expressed as a
/// sequence of contiguous ranges.
#[derive(Debug)]
struct ByteClassElementRanges<'a> {
    elements: ByteClassElements<'a>,
    range: Option<(Unit, Unit)>,
}

impl<'a> Iterator for ByteClassElementRanges<'a> {
    type Item = (Unit, Unit);

    fn next(&mut self) -> Option<(Unit, Unit)> {
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
                    if end.as_usize() + 1 != element.as_usize()
                        || element.is_eoi()
                    {
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
/// For example, in the regex `[ab]+`, the bytes `a` and `b` would be in the
/// same equivalence class because it never matters whether an `a` or a `b` is
/// seen, and no combination of `a`s and `b`s in the text can discriminate a
/// match.
///
/// Note though that this does not compute the minimal set of equivalence
/// classes. For example, in the regex `[ac]+`, both `a` and `c` are in the
/// same equivalence class for the same reason that `a` and `b` are in the
/// same equivalence class in the aforementioned regex. However, in this
/// implementation, `a` and `c` are put into distinct equivalence classes. The
/// reason for this is implementation complexity. In the future, we should
/// endeavor to compute the minimal equivalence classes since they can have a
/// rather large impact on the size of the DFA. (Doing this will likely require
/// rethinking how equivalence classes are computed, including changing the
/// representation here, which is only able to group contiguous bytes into the
/// same equivalence class.)
#[cfg(feature = "alloc")]
#[derive(Clone, Debug)]
pub(crate) struct ByteClassSet(ByteSet);

#[cfg(feature = "alloc")]
impl Default for ByteClassSet {
    fn default() -> ByteClassSet {
        ByteClassSet::empty()
    }
}

#[cfg(feature = "alloc")]
impl ByteClassSet {
    /// Create a new set of byte classes where all bytes are part of the same
    /// equivalence class.
    pub(crate) fn empty() -> Self {
        ByteClassSet(ByteSet::empty())
    }

    /// Indicate the range of byte given (inclusive) can discriminate a
    /// match between it and all other bytes outside of the range.
    pub(crate) fn set_range(&mut self, start: u8, end: u8) {
        debug_assert!(start <= end);
        if start > 0 {
            self.0.add(start - 1);
        }
        self.0.add(end);
    }

    /// Add the contiguous ranges in the set given to this byte class set.
    pub(crate) fn add_set(&mut self, set: &ByteSet) {
        for (start, end) in set.iter_ranges() {
            self.set_range(start, end);
        }
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

    /// Remove a byte from this set.
    ///
    /// If the given byte is not in this set, then this is a no-op.
    pub(crate) fn remove(&mut self, byte: u8) {
        let bucket = byte / 128;
        let bit = byte % 128;
        self.bits.0[usize::from(bucket)] &= !(1 << bit);
    }

    /// Return true if and only if the given byte is in this set.
    pub(crate) fn contains(&self, byte: u8) -> bool {
        let bucket = byte / 128;
        let bit = byte % 128;
        self.bits.0[usize::from(bucket)] & (1 << bit) > 0
    }

    /// Return true if and only if the given inclusive range of bytes is in
    /// this set.
    pub(crate) fn contains_range(&self, start: u8, end: u8) -> bool {
        (start..=end).all(|b| self.contains(b))
    }

    /// Returns an iterator over all bytes in this set.
    pub(crate) fn iter(&self) -> ByteSetIter {
        ByteSetIter { set: self, b: 0 }
    }

    /// Returns an iterator over all contiguous ranges of bytes in this set.
    pub(crate) fn iter_ranges(&self) -> ByteSetRangeIter {
        ByteSetRangeIter { set: self, b: 0 }
    }

    /// Return true if and only if this set is empty.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn is_empty(&self) -> bool {
        self.bits.0 == [0, 0]
    }

    /// Deserializes a byte set from the given slice. If the slice is of
    /// incorrect length or is otherwise malformed, then an error is returned.
    /// Upon success, the number of bytes read along with the set are returned.
    /// The number of bytes read is always a multiple of 8.
    pub(crate) fn from_bytes(
        slice: &[u8],
    ) -> Result<(ByteSet, usize), DeserializeError> {
        use core::mem::size_of;

        wire::check_slice_len(slice, 2 * size_of::<u128>(), "byte set")?;
        let mut nread = 0;
        let (low, nr) = wire::try_read_u128(slice, "byte set low bucket")?;
        nread += nr;
        let (high, nr) = wire::try_read_u128(slice, "byte set high bucket")?;
        nread += nr;
        Ok((ByteSet { bits: BitSet([low, high]) }, nread))
    }

    /// Writes this byte set to the given byte buffer. If the given buffer is
    /// too small, then an error is returned. Upon success, the total number of
    /// bytes written is returned. The number of bytes written is guaranteed to
    /// be a multiple of 8.
    pub(crate) fn write_to<E: crate::util::wire::Endian>(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        use core::mem::size_of;

        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("byte set"));
        }
        let mut nw = 0;
        E::write_u128(self.bits.0[0], &mut dst[nw..]);
        nw += size_of::<u128>();
        E::write_u128(self.bits.0[1], &mut dst[nw..]);
        nw += size_of::<u128>();
        assert_eq!(nwrite, nw, "expected to write certain number of bytes",);
        assert_eq!(
            nw % 8,
            0,
            "expected to write multiple of 8 bytes for byte set",
        );
        Ok(nw)
    }

    /// Returns the total number of bytes written by `write_to`.
    pub(crate) fn write_to_len(&self) -> usize {
        2 * core::mem::size_of::<u128>()
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

#[derive(Debug)]
pub(crate) struct ByteSetIter<'a> {
    set: &'a ByteSet,
    b: usize,
}

impl<'a> Iterator for ByteSetIter<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        while self.b <= 255 {
            let b = u8::try_from(self.b).unwrap();
            self.b += 1;
            if self.set.contains(b) {
                return Some(b);
            }
        }
        None
    }
}

#[derive(Debug)]
pub(crate) struct ByteSetRangeIter<'a> {
    set: &'a ByteSet,
    b: usize,
}

impl<'a> Iterator for ByteSetRangeIter<'a> {
    type Item = (u8, u8);

    fn next(&mut self) -> Option<(u8, u8)> {
        let asu8 = |n: usize| u8::try_from(n).unwrap();
        while self.b <= 255 {
            let start = asu8(self.b);
            self.b += 1;
            if !self.set.contains(start) {
                continue;
            }

            let mut end = start;
            while self.b <= 255 && self.set.contains(asu8(self.b)) {
                end = asu8(self.b);
                self.b += 1;
            }
            return Some((start, end));
        }
        None
    }
}

#[cfg(all(test, feature = "alloc"))]
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
        assert_eq!(set.byte_classes().alphabet_len(), 257);
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
        // class 7: EOI
        assert_eq!(classes.alphabet_len(), 8);

        let elements = classes.elements(Unit::u8(0)).collect::<Vec<_>>();
        assert_eq!(elements.len(), 98);
        assert_eq!(elements[0], Unit::u8(b'\x00'));
        assert_eq!(elements[97], Unit::u8(b'a'));

        let elements = classes.elements(Unit::u8(1)).collect::<Vec<_>>();
        assert_eq!(
            elements,
            vec![Unit::u8(b'b'), Unit::u8(b'c'), Unit::u8(b'd')],
        );

        let elements = classes.elements(Unit::u8(2)).collect::<Vec<_>>();
        assert_eq!(elements, vec![Unit::u8(b'e'), Unit::u8(b'f')],);

        let elements = classes.elements(Unit::u8(3)).collect::<Vec<_>>();
        assert_eq!(
            elements,
            vec![
                Unit::u8(b'g'),
                Unit::u8(b'h'),
                Unit::u8(b'i'),
                Unit::u8(b'j'),
                Unit::u8(b'k'),
                Unit::u8(b'l'),
                Unit::u8(b'm'),
            ],
        );

        let elements = classes.elements(Unit::u8(4)).collect::<Vec<_>>();
        assert_eq!(elements.len(), 12);
        assert_eq!(elements[0], Unit::u8(b'n'));
        assert_eq!(elements[11], Unit::u8(b'y'));

        let elements = classes.elements(Unit::u8(5)).collect::<Vec<_>>();
        assert_eq!(elements, vec![Unit::u8(b'z')]);

        let elements = classes.elements(Unit::u8(6)).collect::<Vec<_>>();
        assert_eq!(elements.len(), 133);
        assert_eq!(elements[0], Unit::u8(b'\x7B'));
        assert_eq!(elements[132], Unit::u8(b'\xFF'));

        let elements = classes.elements(Unit::eoi(7)).collect::<Vec<_>>();
        assert_eq!(elements, vec![Unit::eoi(256)]);
    }

    #[test]
    fn elements_singletons() {
        let classes = ByteClasses::singletons();
        assert_eq!(classes.alphabet_len(), 257);

        let elements = classes.elements(Unit::u8(b'a')).collect::<Vec<_>>();
        assert_eq!(elements, vec![Unit::u8(b'a')]);

        let elements = classes.elements(Unit::eoi(5)).collect::<Vec<_>>();
        assert_eq!(elements, vec![Unit::eoi(256)]);
    }

    #[test]
    fn elements_empty() {
        let classes = ByteClasses::empty();
        assert_eq!(classes.alphabet_len(), 2);

        let elements = classes.elements(Unit::u8(0)).collect::<Vec<_>>();
        assert_eq!(elements.len(), 256);
        assert_eq!(elements[0], Unit::u8(b'\x00'));
        assert_eq!(elements[255], Unit::u8(b'\xFF'));

        let elements = classes.elements(Unit::eoi(1)).collect::<Vec<_>>();
        assert_eq!(elements, vec![Unit::eoi(256)]);
    }

    #[test]
    fn representatives() {
        let mut set = ByteClassSet::empty();
        set.set_range(b'b', b'd');
        set.set_range(b'g', b'm');
        set.set_range(b'z', b'z');
        let classes = set.byte_classes();

        let got: Vec<Unit> = classes.representatives(..).collect();
        let expected = vec![
            Unit::u8(b'\x00'),
            Unit::u8(b'b'),
            Unit::u8(b'e'),
            Unit::u8(b'g'),
            Unit::u8(b'n'),
            Unit::u8(b'z'),
            Unit::u8(b'\x7B'),
            Unit::eoi(7),
        ];
        assert_eq!(expected, got);

        let got: Vec<Unit> = classes.representatives(..0).collect();
        assert!(got.is_empty());
        let got: Vec<Unit> = classes.representatives(1..1).collect();
        assert!(got.is_empty());
        let got: Vec<Unit> = classes.representatives(255..255).collect();
        assert!(got.is_empty());

        // A weird case that is the only guaranteed to way to get an iterator
        // of just the EOI class by excluding all possible byte values.
        let got: Vec<Unit> = classes
            .representatives((
                core::ops::Bound::Excluded(255),
                core::ops::Bound::Unbounded,
            ))
            .collect();
        let expected = vec![Unit::eoi(7)];
        assert_eq!(expected, got);

        let got: Vec<Unit> = classes.representatives(..=255).collect();
        let expected = vec![
            Unit::u8(b'\x00'),
            Unit::u8(b'b'),
            Unit::u8(b'e'),
            Unit::u8(b'g'),
            Unit::u8(b'n'),
            Unit::u8(b'z'),
            Unit::u8(b'\x7B'),
        ];
        assert_eq!(expected, got);

        let got: Vec<Unit> = classes.representatives(b'b'..=b'd').collect();
        let expected = vec![Unit::u8(b'b')];
        assert_eq!(expected, got);

        let got: Vec<Unit> = classes.representatives(b'a'..=b'd').collect();
        let expected = vec![Unit::u8(b'a'), Unit::u8(b'b')];
        assert_eq!(expected, got);

        let got: Vec<Unit> = classes.representatives(b'b'..=b'e').collect();
        let expected = vec![Unit::u8(b'b'), Unit::u8(b'e')];
        assert_eq!(expected, got);

        let got: Vec<Unit> = classes.representatives(b'A'..=b'Z').collect();
        let expected = vec![Unit::u8(b'A')];
        assert_eq!(expected, got);

        let got: Vec<Unit> = classes.representatives(b'A'..=b'z').collect();
        let expected = vec![
            Unit::u8(b'A'),
            Unit::u8(b'b'),
            Unit::u8(b'e'),
            Unit::u8(b'g'),
            Unit::u8(b'n'),
            Unit::u8(b'z'),
        ];
        assert_eq!(expected, got);

        let got: Vec<Unit> = classes.representatives(b'z'..).collect();
        let expected = vec![Unit::u8(b'z'), Unit::u8(b'\x7B'), Unit::eoi(7)];
        assert_eq!(expected, got);

        let got: Vec<Unit> = classes.representatives(b'z'..=0xFF).collect();
        let expected = vec![Unit::u8(b'z'), Unit::u8(b'\x7B')];
        assert_eq!(expected, got);
    }
}
