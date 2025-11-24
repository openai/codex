// This module defines some core types for dealing with accelerated DFA states.
// Briefly, a DFA state can be "accelerated" if all of its transitions except
// for a few loop back to itself. This directly implies that the only way out
// of such a state is if a byte corresponding to one of those non-loopback
// transitions is found. Such states are often found in simple repetitions in
// non-Unicode regexes. For example, consider '(?-u)[^a]+a'. We can look at its
// DFA with regex-cli:
//
//     $ regex-cli debug dense dfa -p '(?-u)[^a]+a' -BbC --no-table
//     D 000000:
//     Q 000001:
//      *000002:
//     A 000003: \x00-` => 3, a => 8, b-\xFF => 3
//     A 000004: \x00-` => 4, a => 7, b-\xFF => 4
//       000005: \x00-` => 4, b-\xFF => 4
//       000006: \x00-` => 3, a => 6, b-\xFF => 3
//       000007: \x00-\xFF => 2, EOI => 2
//       000008: \x00-\xFF => 2, EOI => 2
//
// In particular, state 3 is accelerated (shown via the 'A' indicator) since
// the only way to leave that state once entered is to see an 'a' byte. If
// there is a long run of non-'a' bytes, then using something like 'memchr'
// to find the next 'a' byte can be significantly faster than just using the
// standard byte-at-a-time state machine.
//
// Unfortunately, this optimization rarely applies when Unicode is enabled.
// For example, patterns like '[^a]' don't actually match any byte that isn't
// 'a', but rather, any UTF-8 encoding of a Unicode scalar value that isn't
// 'a'. This makes the state machine much more complex---far beyond a single
// state---and removes the ability to easily accelerate it. (Because if the
// machine sees a non-UTF-8 sequence, then the machine won't match through it.)
//
// In practice, we only consider accelerating states that have 3 or fewer
// non-loop transitions. At a certain point, you get diminishing returns, but
// also because that's what the memchr crate supports. The structures below
// hard-code this assumption and provide (de)serialization APIs for use inside
// a DFA.
//
// And finally, note that there is some trickery involved in making it very
// fast to not only check whether a state is accelerated at search time, but
// also to access the bytes to search for to implement the acceleration itself.
// dfa/special.rs provides more detail, but the short story is that all
// accelerated states appear contiguously in a DFA. This means we can represent
// the ID space of all accelerated DFA states with a single range. So given
// a state ID, we can determine whether it's accelerated via
//
//     min_accel_id <= id <= max_accel_id
//
// And find its corresponding accelerator with:
//
//     accels.get((id - min_accel_id) / dfa_stride)

#[cfg(feature = "dfa-build")]
use alloc::{vec, vec::Vec};

use crate::util::{
    int::Pointer,
    memchr,
    wire::{self, DeserializeError, Endian, SerializeError},
};

/// The base type used to represent a collection of accelerators.
///
/// While an `Accel` is represented as a fixed size array of bytes, a
/// *collection* of `Accel`s (called `Accels`) is represented internally as a
/// slice of u32. While it's a bit unnatural to do this and costs us a bit of
/// fairly low-risk not-safe code, it lets us remove the need for a second type
/// parameter in the definition of dense::DFA. (Which really wants everything
/// to be a slice of u32.)
type AccelTy = u32;

/// The size of the unit of representation for accelerators.
///
/// ACCEL_CAP *must* be a multiple of this size.
const ACCEL_TY_SIZE: usize = core::mem::size_of::<AccelTy>();

/// The maximum length in bytes that a single Accel can be. This is distinct
/// from the capacity of an accelerator in that the length represents only the
/// bytes that should be read.
const ACCEL_LEN: usize = 4;

/// The capacity of each accelerator, in bytes. We set this to 8 since it's a
/// multiple of 4 (our ID size) and because it gives us a little wiggle room
/// if we want to support more accel bytes in the future without a breaking
/// change.
///
/// This MUST be a multiple of ACCEL_TY_SIZE.
const ACCEL_CAP: usize = 8;

/// Search for between 1 and 3 needle bytes in the given haystack, starting the
/// search at the given position. If `needles` has a length other than 1-3,
/// then this panics.
#[cfg_attr(feature = "perf-inline", inline(always))]
pub(crate) fn find_fwd(
    needles: &[u8],
    haystack: &[u8],
    at: usize,
) -> Option<usize> {
    let bs = needles;
    let i = match needles.len() {
        1 => memchr::memchr(bs[0], &haystack[at..])?,
        2 => memchr::memchr2(bs[0], bs[1], &haystack[at..])?,
        3 => memchr::memchr3(bs[0], bs[1], bs[2], &haystack[at..])?,
        0 => panic!("cannot find with empty needles"),
        n => panic!("invalid needles length: {}", n),
    };
    Some(at + i)
}

/// Search for between 1 and 3 needle bytes in the given haystack in reverse,
/// starting the search at the given position. If `needles` has a length other
/// than 1-3, then this panics.
#[cfg_attr(feature = "perf-inline", inline(always))]
pub(crate) fn find_rev(
    needles: &[u8],
    haystack: &[u8],
    at: usize,
) -> Option<usize> {
    let bs = needles;
    match needles.len() {
        1 => memchr::memrchr(bs[0], &haystack[..at]),
        2 => memchr::memrchr2(bs[0], bs[1], &haystack[..at]),
        3 => memchr::memrchr3(bs[0], bs[1], bs[2], &haystack[..at]),
        0 => panic!("cannot find with empty needles"),
        n => panic!("invalid needles length: {}", n),
    }
}

/// Represents the accelerators for all accelerated states in a dense DFA.
///
/// The `A` type parameter represents the type of the underlying bytes.
/// Generally, this is either `&[AccelTy]` or `Vec<AccelTy>`.
#[derive(Clone)]
pub(crate) struct Accels<A> {
    /// A length prefixed slice of contiguous accelerators. See the top comment
    /// in this module for more details on how we can jump from a DFA's state
    /// ID to an accelerator in this list.
    ///
    /// The first 4 bytes always correspond to the number of accelerators
    /// that follow.
    accels: A,
}

#[cfg(feature = "dfa-build")]
impl Accels<Vec<AccelTy>> {
    /// Create an empty sequence of accelerators for a DFA.
    pub fn empty() -> Accels<Vec<AccelTy>> {
        Accels { accels: vec![0] }
    }

    /// Add an accelerator to this sequence.
    ///
    /// This adds to the accelerator to the end of the sequence and therefore
    /// should be done in correspondence with its state in the DFA.
    ///
    /// This panics if this results in more accelerators than AccelTy::MAX.
    pub fn add(&mut self, accel: Accel) {
        self.accels.extend_from_slice(&accel.as_accel_tys());
        let len = self.len();
        self.set_len(len + 1);
    }

    /// Set the number of accelerators in this sequence, which is encoded in
    /// the first 4 bytes of the underlying bytes.
    fn set_len(&mut self, new_len: usize) {
        // The only way an accelerator gets added is if a state exists for
        // it, and if a state exists, then its index is guaranteed to be
        // representable by a AccelTy by virtue of the guarantees provided by
        // StateID.
        let new_len = AccelTy::try_from(new_len).unwrap();
        self.accels[0] = new_len;
    }
}

impl<'a> Accels<&'a [AccelTy]> {
    /// Deserialize a sequence of accelerators from the given bytes. If there
    /// was a problem deserializing, then an error is returned.
    ///
    /// This is guaranteed to run in constant time. This does not guarantee
    /// that every accelerator in the returned collection is valid. Thus,
    /// accessing one may panic, or not-safe code that relies on accelerators
    /// being correct my result in UB.
    ///
    /// Callers may check the validity of every accelerator with the `validate`
    /// method.
    pub fn from_bytes_unchecked(
        mut slice: &'a [u8],
    ) -> Result<(Accels<&'a [AccelTy]>, usize), DeserializeError> {
        let slice_start = slice.as_ptr().as_usize();

        let (accel_len, _) =
            wire::try_read_u32_as_usize(slice, "accelerators length")?;
        // The accelerator length is part of the accel_tys slice that
        // we deserialize. This is perhaps a bit idiosyncratic. It would
        // probably be better to split out the length into a real field.

        let accel_tys_len = wire::add(
            wire::mul(accel_len, 2, "total number of accelerator accel_tys")?,
            1,
            "total number of accel_tys",
        )?;
        let accel_tys_bytes_len = wire::mul(
            ACCEL_TY_SIZE,
            accel_tys_len,
            "total number of bytes in accelerators",
        )?;
        wire::check_slice_len(slice, accel_tys_bytes_len, "accelerators")?;
        wire::check_alignment::<AccelTy>(slice)?;
        let accel_tys = &slice[..accel_tys_bytes_len];
        slice = &slice[accel_tys_bytes_len..];
        // SAFETY: We've checked the length and alignment above, and since
        // slice is just bytes and AccelTy is just a u32, we can safely cast to
        // a slice of &[AccelTy].
        let accels = unsafe {
            core::slice::from_raw_parts(
                accel_tys.as_ptr().cast::<AccelTy>(),
                accel_tys_len,
            )
        };
        Ok((Accels { accels }, slice.as_ptr().as_usize() - slice_start))
    }
}

impl<A: AsRef<[AccelTy]>> Accels<A> {
    /// Return an owned version of the accelerators.
    #[cfg(feature = "alloc")]
    pub fn to_owned(&self) -> Accels<alloc::vec::Vec<AccelTy>> {
        Accels { accels: self.accels.as_ref().to_vec() }
    }

    /// Return a borrowed version of the accelerators.
    pub fn as_ref(&self) -> Accels<&[AccelTy]> {
        Accels { accels: self.accels.as_ref() }
    }

    /// Return the bytes representing the serialization of the accelerators.
    pub fn as_bytes(&self) -> &[u8] {
        let accels = self.accels.as_ref();
        // SAFETY: This is safe because accels is a just a slice of AccelTy,
        // and u8 always has a smaller alignment.
        unsafe {
            core::slice::from_raw_parts(
                accels.as_ptr().cast::<u8>(),
                accels.len() * ACCEL_TY_SIZE,
            )
        }
    }

    /// Returns the memory usage, in bytes, of these accelerators.
    ///
    /// The memory usage is computed based on the number of bytes used to
    /// represent all of the accelerators.
    ///
    /// This does **not** include the stack size used by this value.
    pub fn memory_usage(&self) -> usize {
        self.as_bytes().len()
    }

    /// Return the bytes to search for corresponding to the accelerator in this
    /// sequence at index `i`. If no such accelerator exists, then this panics.
    ///
    /// The significance of the index is that it should be in correspondence
    /// with the index of the corresponding DFA. That is, accelerated DFA
    /// states are stored contiguously in the DFA and have an ordering implied
    /// by their respective state IDs. The state's index in that sequence
    /// corresponds to the index of its corresponding accelerator.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub fn needles(&self, i: usize) -> &[u8] {
        if i >= self.len() {
            panic!("invalid accelerator index {}", i);
        }
        let bytes = self.as_bytes();
        let offset = ACCEL_TY_SIZE + i * ACCEL_CAP;
        let len = usize::from(bytes[offset]);
        &bytes[offset + 1..offset + 1 + len]
    }

    /// Return the total number of accelerators in this sequence.
    pub fn len(&self) -> usize {
        // This should never panic since deserialization checks that the
        // length can fit into a usize.
        usize::try_from(self.accels.as_ref()[0]).unwrap()
    }

    /// Return the accelerator in this sequence at index `i`. If no such
    /// accelerator exists, then this returns None.
    ///
    /// See the docs for `needles` on the significance of the index.
    fn get(&self, i: usize) -> Option<Accel> {
        if i >= self.len() {
            return None;
        }
        let offset = ACCEL_TY_SIZE + i * ACCEL_CAP;
        let accel = Accel::from_slice(&self.as_bytes()[offset..])
            .expect("Accels must contain valid accelerators");
        Some(accel)
    }

    /// Returns an iterator of accelerators in this sequence.
    fn iter(&self) -> IterAccels<'_, A> {
        IterAccels { accels: self, i: 0 }
    }

    /// Writes these accelerators to the given byte buffer using the indicated
    /// endianness. If the given buffer is too small, then an error is
    /// returned. Upon success, the total number of bytes written is returned.
    /// The number of bytes written is guaranteed to be a multiple of 8.
    pub fn write_to<E: Endian>(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        assert_eq!(
            nwrite % ACCEL_TY_SIZE,
            0,
            "expected accelerator bytes written to be a multiple of {}",
            ACCEL_TY_SIZE,
        );
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("accelerators"));
        }

        // The number of accelerators can never exceed AccelTy::MAX.
        E::write_u32(AccelTy::try_from(self.len()).unwrap(), dst);
        // The actual accelerators are just raw bytes and thus their endianness
        // is irrelevant. So we can copy them as bytes.
        dst[ACCEL_TY_SIZE..nwrite]
            .copy_from_slice(&self.as_bytes()[ACCEL_TY_SIZE..nwrite]);
        Ok(nwrite)
    }

    /// Validates that every accelerator in this collection can be successfully
    /// deserialized as a valid accelerator.
    pub fn validate(&self) -> Result<(), DeserializeError> {
        for chunk in self.as_bytes()[ACCEL_TY_SIZE..].chunks(ACCEL_CAP) {
            let _ = Accel::from_slice(chunk)?;
        }
        Ok(())
    }

    /// Returns the total number of bytes written by `write_to`.
    pub fn write_to_len(&self) -> usize {
        self.as_bytes().len()
    }
}

impl<A: AsRef<[AccelTy]>> core::fmt::Debug for Accels<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Accels(")?;
        let mut list = f.debug_list();
        for a in self.iter() {
            list.entry(&a);
        }
        list.finish()?;
        write!(f, ")")
    }
}

#[derive(Debug)]
struct IterAccels<'a, A: AsRef<[AccelTy]>> {
    accels: &'a Accels<A>,
    i: usize,
}

impl<'a, A: AsRef<[AccelTy]>> Iterator for IterAccels<'a, A> {
    type Item = Accel;

    fn next(&mut self) -> Option<Accel> {
        let accel = self.accels.get(self.i)?;
        self.i += 1;
        Some(accel)
    }
}

/// Accel represents a structure for determining how to "accelerate" a DFA
/// state.
///
/// Namely, it contains zero or more bytes that must be seen in order for the
/// DFA to leave the state it is associated with. In practice, the actual range
/// is 1 to 3 bytes.
///
/// The purpose of acceleration is to identify states whose vast majority
/// of transitions are just loops back to the same state. For example,
/// in the regex `(?-u)^[^a]+b`, the corresponding DFA will have a state
/// (corresponding to `[^a]+`) where all transitions *except* for `a` and
/// `b` loop back to itself. Thus, this state can be "accelerated" by simply
/// looking for the next occurrence of either `a` or `b` instead of explicitly
/// following transitions. (In this case, `b` transitions to the next state
/// where as `a` would transition to the dead state.)
#[derive(Clone)]
pub(crate) struct Accel {
    /// The first byte is the length. Subsequent bytes are the accelerated
    /// bytes.
    ///
    /// Note that we make every accelerator 8 bytes as a slightly wasteful
    /// way of making sure alignment is always correct for state ID sizes of
    /// 1, 2, 4 and 8. This should be okay since accelerated states aren't
    /// particularly common, especially when Unicode is enabled.
    bytes: [u8; ACCEL_CAP],
}

impl Accel {
    /// Returns an empty accel, where no bytes are accelerated.
    #[cfg(feature = "dfa-build")]
    pub fn new() -> Accel {
        Accel { bytes: [0; ACCEL_CAP] }
    }

    /// Returns a verified accelerator derived from the beginning of the given
    /// slice.
    ///
    /// If the slice is not long enough or contains invalid bytes for an
    /// accelerator, then this returns an error.
    pub fn from_slice(mut slice: &[u8]) -> Result<Accel, DeserializeError> {
        slice = &slice[..core::cmp::min(ACCEL_LEN, slice.len())];
        let bytes = slice
            .try_into()
            .map_err(|_| DeserializeError::buffer_too_small("accelerator"))?;
        Accel::from_bytes(bytes)
    }

    /// Returns a verified accelerator derived from raw bytes.
    ///
    /// If the given bytes are invalid, then this returns an error.
    fn from_bytes(bytes: [u8; 4]) -> Result<Accel, DeserializeError> {
        if usize::from(bytes[0]) >= ACCEL_LEN {
            return Err(DeserializeError::generic(
                "accelerator bytes cannot have length more than 3",
            ));
        }
        Ok(Accel::from_bytes_unchecked(bytes))
    }

    /// Returns an accelerator derived from raw bytes.
    ///
    /// This does not check whether the given bytes are valid. Invalid bytes
    /// cannot sacrifice memory safety, but may result in panics or silent
    /// logic bugs.
    fn from_bytes_unchecked(bytes: [u8; 4]) -> Accel {
        Accel { bytes: [bytes[0], bytes[1], bytes[2], bytes[3], 0, 0, 0, 0] }
    }

    /// Attempts to add the given byte to this accelerator. If the accelerator
    /// is already full or thinks the byte is a poor accelerator, then this
    /// returns false. Otherwise, returns true.
    ///
    /// If the given byte is already in this accelerator, then it panics.
    #[cfg(feature = "dfa-build")]
    pub fn add(&mut self, byte: u8) -> bool {
        if self.len() >= 3 {
            return false;
        }
        // As a special case, we totally reject trying to accelerate a state
        // with an ASCII space. In most cases, it occurs very frequently, and
        // tends to result in worse overall performance.
        if byte == b' ' {
            return false;
        }
        assert!(
            !self.contains(byte),
            "accelerator already contains {:?}",
            crate::util::escape::DebugByte(byte)
        );
        self.bytes[self.len() + 1] = byte;
        self.bytes[0] += 1;
        true
    }

    /// Return the number of bytes in this accelerator.
    pub fn len(&self) -> usize {
        usize::from(self.bytes[0])
    }

    /// Returns true if and only if there are no bytes in this accelerator.
    #[cfg(feature = "dfa-build")]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the slice of bytes to accelerate.
    ///
    /// If this accelerator is empty, then this returns an empty slice.
    fn needles(&self) -> &[u8] {
        &self.bytes[1..1 + self.len()]
    }

    /// Returns true if and only if this accelerator will accelerate the given
    /// byte.
    #[cfg(feature = "dfa-build")]
    fn contains(&self, byte: u8) -> bool {
        self.needles().iter().position(|&b| b == byte).is_some()
    }

    /// Returns the accelerator bytes as an array of AccelTys.
    #[cfg(feature = "dfa-build")]
    fn as_accel_tys(&self) -> [AccelTy; 2] {
        assert_eq!(ACCEL_CAP, 8);
        // These unwraps are OK since ACCEL_CAP is set to 8.
        let first =
            AccelTy::from_ne_bytes(self.bytes[0..4].try_into().unwrap());
        let second =
            AccelTy::from_ne_bytes(self.bytes[4..8].try_into().unwrap());
        [first, second]
    }
}

impl core::fmt::Debug for Accel {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Accel(")?;
        let mut set = f.debug_set();
        for &b in self.needles() {
            set.entry(&crate::util::escape::DebugByte(b));
        }
        set.finish()?;
        write!(f, ")")
    }
}
