/*!
Types and routines that support the wire format of finite automata.

Currently, this module just exports a few error types and some small helpers
for deserializing [dense DFAs](crate::dfa::dense::DFA) using correct alignment.
*/

/*
A collection of helper functions, types and traits for serializing automata.

This crate defines its own bespoke serialization mechanism for some structures
provided in the public API, namely, DFAs. A bespoke mechanism was developed
primarily because structures like automata demand a specific binary format.
Attempting to encode their rich structure in an existing serialization
format is just not feasible. Moreover, the format for each structure is
generally designed such that deserialization is cheap. More specifically, that
deserialization can be done in constant time. (The idea being that you can
embed it into your binary or mmap it, and then use it immediately.)

In order to achieve this, the dense and sparse DFAs in this crate use an
in-memory representation that very closely corresponds to its binary serialized
form. This pervades and complicates everything, and in some cases, requires
dealing with alignment and reasoning about safety.

This technique does have major advantages. In particular, it permits doing
the potentially costly work of compiling a finite state machine in an offline
manner, and then loading it at runtime not only without having to re-compile
the regex, but even without the code required to do the compilation. This, for
example, permits one to use a pre-compiled DFA not only in environments without
Rust's standard library, but also in environments without a heap.

In the code below, whenever we insert some kind of padding, it's to enforce a
4-byte alignment, unless otherwise noted. Namely, u32 is the only state ID type
supported. (In a previous version of this library, DFAs were generic over the
state ID representation.)

Also, serialization generally requires the caller to specify endianness,
where as deserialization always assumes native endianness (otherwise cheap
deserialization would be impossible). This implies that serializing a structure
generally requires serializing both its big-endian and little-endian variants,
and then loading the correct one based on the target's endianness.
*/

use core::{cmp, mem::size_of};

#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};

use crate::util::{
    int::Pointer,
    primitives::{PatternID, PatternIDError, StateID, StateIDError},
};

/// A hack to align a smaller type `B` with a bigger type `T`.
///
/// The usual use of this is with `B = [u8]` and `T = u32`. That is,
/// it permits aligning a sequence of bytes on a 4-byte boundary. This
/// is useful in contexts where one wants to embed a serialized [dense
/// DFA](crate::dfa::dense::DFA) into a Rust a program while guaranteeing the
/// alignment required for the DFA.
///
/// See [`dense::DFA::from_bytes`](crate::dfa::dense::DFA::from_bytes) for an
/// example of how to use this type.
#[repr(C)]
#[derive(Debug)]
pub struct AlignAs<B: ?Sized, T> {
    /// A zero-sized field indicating the alignment we want.
    pub _align: [T; 0],
    /// A possibly non-sized field containing a sequence of bytes.
    pub bytes: B,
}

/// An error that occurs when serializing an object from this crate.
///
/// Serialization, as used in this crate, universally refers to the process
/// of transforming a structure (like a DFA) into a custom binary format
/// represented by `&[u8]`. To this end, serialization is generally infallible.
/// However, it can fail when caller provided buffer sizes are too small. When
/// that occurs, a serialization error is reported.
///
/// A `SerializeError` provides no introspection capabilities. Its only
/// supported operation is conversion to a human readable error message.
///
/// This error type implements the `std::error::Error` trait only when the
/// `std` feature is enabled. Otherwise, this type is defined in all
/// configurations.
#[derive(Debug)]
pub struct SerializeError {
    /// The name of the thing that a buffer is too small for.
    ///
    /// Currently, the only kind of serialization error is one that is
    /// committed by a caller: providing a destination buffer that is too
    /// small to fit the serialized object. This makes sense conceptually,
    /// since every valid inhabitant of a type should be serializable.
    ///
    /// This is somewhat exposed in the public API of this crate. For example,
    /// the `to_bytes_{big,little}_endian` APIs return a `Vec<u8>` and are
    /// guaranteed to never panic or error. This is only possible because the
    /// implementation guarantees that it will allocate a `Vec<u8>` that is
    /// big enough.
    ///
    /// In summary, if a new serialization error kind needs to be added, then
    /// it will need careful consideration.
    what: &'static str,
}

impl SerializeError {
    pub(crate) fn buffer_too_small(what: &'static str) -> SerializeError {
        SerializeError { what }
    }
}

impl core::fmt::Display for SerializeError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "destination buffer is too small to write {}", self.what)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SerializeError {}

/// An error that occurs when deserializing an object defined in this crate.
///
/// Serialization, as used in this crate, universally refers to the process
/// of transforming a structure (like a DFA) into a custom binary format
/// represented by `&[u8]`. Deserialization, then, refers to the process of
/// cheaply converting this binary format back to the object's in-memory
/// representation as defined in this crate. To the extent possible,
/// deserialization will report this error whenever this process fails.
///
/// A `DeserializeError` provides no introspection capabilities. Its only
/// supported operation is conversion to a human readable error message.
///
/// This error type implements the `std::error::Error` trait only when the
/// `std` feature is enabled. Otherwise, this type is defined in all
/// configurations.
#[derive(Debug)]
pub struct DeserializeError(DeserializeErrorKind);

#[derive(Debug)]
enum DeserializeErrorKind {
    Generic { msg: &'static str },
    BufferTooSmall { what: &'static str },
    InvalidUsize { what: &'static str },
    VersionMismatch { expected: u32, found: u32 },
    EndianMismatch { expected: u32, found: u32 },
    AlignmentMismatch { alignment: usize, address: usize },
    LabelMismatch { expected: &'static str },
    ArithmeticOverflow { what: &'static str },
    PatternID { err: PatternIDError, what: &'static str },
    StateID { err: StateIDError, what: &'static str },
}

impl DeserializeError {
    pub(crate) fn generic(msg: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::Generic { msg })
    }

    pub(crate) fn buffer_too_small(what: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::BufferTooSmall { what })
    }

    fn invalid_usize(what: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::InvalidUsize { what })
    }

    fn version_mismatch(expected: u32, found: u32) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::VersionMismatch {
            expected,
            found,
        })
    }

    fn endian_mismatch(expected: u32, found: u32) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::EndianMismatch {
            expected,
            found,
        })
    }

    fn alignment_mismatch(
        alignment: usize,
        address: usize,
    ) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::AlignmentMismatch {
            alignment,
            address,
        })
    }

    fn label_mismatch(expected: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::LabelMismatch { expected })
    }

    fn arithmetic_overflow(what: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::ArithmeticOverflow { what })
    }

    fn pattern_id_error(
        err: PatternIDError,
        what: &'static str,
    ) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::PatternID { err, what })
    }

    pub(crate) fn state_id_error(
        err: StateIDError,
        what: &'static str,
    ) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::StateID { err, what })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DeserializeError {}

impl core::fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use self::DeserializeErrorKind::*;

        match self.0 {
            Generic { msg } => write!(f, "{}", msg),
            BufferTooSmall { what } => {
                write!(f, "buffer is too small to read {}", what)
            }
            InvalidUsize { what } => {
                write!(f, "{} is too big to fit in a usize", what)
            }
            VersionMismatch { expected, found } => write!(
                f,
                "unsupported version: \
                 expected version {} but found version {}",
                expected, found,
            ),
            EndianMismatch { expected, found } => write!(
                f,
                "endianness mismatch: expected 0x{:X} but got 0x{:X}. \
                 (Are you trying to load an object serialized with a \
                 different endianness?)",
                expected, found,
            ),
            AlignmentMismatch { alignment, address } => write!(
                f,
                "alignment mismatch: slice starts at address \
                 0x{:X}, which is not aligned to a {} byte boundary",
                address, alignment,
            ),
            LabelMismatch { expected } => write!(
                f,
                "label mismatch: start of serialized object should \
                 contain a NUL terminated {:?} label, but a different \
                 label was found",
                expected,
            ),
            ArithmeticOverflow { what } => {
                write!(f, "arithmetic overflow for {}", what)
            }
            PatternID { ref err, what } => {
                write!(f, "failed to read pattern ID for {}: {}", what, err)
            }
            StateID { ref err, what } => {
                write!(f, "failed to read state ID for {}: {}", what, err)
            }
        }
    }
}

/// Safely converts a `&[u32]` to `&[StateID]` with zero cost.
#[cfg_attr(feature = "perf-inline", inline(always))]
pub(crate) fn u32s_to_state_ids(slice: &[u32]) -> &[StateID] {
    // SAFETY: This is safe because StateID is defined to have the same memory
    // representation as a u32 (it is repr(transparent)). While not every u32
    // is a "valid" StateID, callers are not permitted to rely on the validity
    // of StateIDs for memory safety. It can only lead to logical errors. (This
    // is why StateID::new_unchecked is safe.)
    unsafe {
        core::slice::from_raw_parts(
            slice.as_ptr().cast::<StateID>(),
            slice.len(),
        )
    }
}

/// Safely converts a `&mut [u32]` to `&mut [StateID]` with zero cost.
pub(crate) fn u32s_to_state_ids_mut(slice: &mut [u32]) -> &mut [StateID] {
    // SAFETY: This is safe because StateID is defined to have the same memory
    // representation as a u32 (it is repr(transparent)). While not every u32
    // is a "valid" StateID, callers are not permitted to rely on the validity
    // of StateIDs for memory safety. It can only lead to logical errors. (This
    // is why StateID::new_unchecked is safe.)
    unsafe {
        core::slice::from_raw_parts_mut(
            slice.as_mut_ptr().cast::<StateID>(),
            slice.len(),
        )
    }
}

/// Safely converts a `&[u32]` to `&[PatternID]` with zero cost.
#[cfg_attr(feature = "perf-inline", inline(always))]
pub(crate) fn u32s_to_pattern_ids(slice: &[u32]) -> &[PatternID] {
    // SAFETY: This is safe because PatternID is defined to have the same
    // memory representation as a u32 (it is repr(transparent)). While not
    // every u32 is a "valid" PatternID, callers are not permitted to rely
    // on the validity of PatternIDs for memory safety. It can only lead to
    // logical errors. (This is why PatternID::new_unchecked is safe.)
    unsafe {
        core::slice::from_raw_parts(
            slice.as_ptr().cast::<PatternID>(),
            slice.len(),
        )
    }
}

/// Checks that the given slice has an alignment that matches `T`.
///
/// This is useful for checking that a slice has an appropriate alignment
/// before casting it to a &[T]. Note though that alignment is not itself
/// sufficient to perform the cast for any `T`.
pub(crate) fn check_alignment<T>(
    slice: &[u8],
) -> Result<(), DeserializeError> {
    let alignment = core::mem::align_of::<T>();
    let address = slice.as_ptr().as_usize();
    if address % alignment == 0 {
        return Ok(());
    }
    Err(DeserializeError::alignment_mismatch(alignment, address))
}

/// Reads a possibly empty amount of padding, up to 7 bytes, from the beginning
/// of the given slice. All padding bytes must be NUL bytes.
///
/// This is useful because it can be theoretically necessary to pad the
/// beginning of a serialized object with NUL bytes to ensure that it starts
/// at a correctly aligned address. These padding bytes should come immediately
/// before the label.
///
/// This returns the number of bytes read from the given slice.
pub(crate) fn skip_initial_padding(slice: &[u8]) -> usize {
    let mut nread = 0;
    while nread < 7 && nread < slice.len() && slice[nread] == 0 {
        nread += 1;
    }
    nread
}

/// Allocate a byte buffer of the given size, along with some initial padding
/// such that `buf[padding..]` has the same alignment as `T`, where the
/// alignment of `T` must be at most `8`. In particular, callers should treat
/// the first N bytes (second return value) as padding bytes that must not be
/// overwritten. In all cases, the following identity holds:
///
/// ```ignore
/// let (buf, padding) = alloc_aligned_buffer::<StateID>(SIZE);
/// assert_eq!(SIZE, buf[padding..].len());
/// ```
///
/// In practice, padding is often zero.
///
/// The requirement for `8` as a maximum here is somewhat arbitrary. In
/// practice, we never need anything bigger in this crate, and so this function
/// does some sanity asserts under the assumption of a max alignment of `8`.
#[cfg(feature = "alloc")]
pub(crate) fn alloc_aligned_buffer<T>(size: usize) -> (Vec<u8>, usize) {
    // NOTE: This is a kludge because there's no easy way to allocate a Vec<u8>
    // with an alignment guaranteed to be greater than 1. We could create a
    // Vec<u32>, but this cannot be safely transmuted to a Vec<u8> without
    // concern, since reallocing or dropping the Vec<u8> is UB (different
    // alignment than the initial allocation). We could define a wrapper type
    // to manage this for us, but it seems like more machinery than it's worth.
    let buf = vec![0; size];
    let align = core::mem::align_of::<T>();
    let address = buf.as_ptr().as_usize();
    if address % align == 0 {
        return (buf, 0);
    }
    // Let's try this again. We have to create a totally new alloc with
    // the maximum amount of bytes we might need. We can't just extend our
    // pre-existing 'buf' because that might create a new alloc with a
    // different alignment.
    let extra = align - 1;
    let mut buf = vec![0; size + extra];
    let address = buf.as_ptr().as_usize();
    // The code below handles the case where 'address' is aligned to T, so if
    // we got lucky and 'address' is now aligned to T (when it previously
    // wasn't), then we're done.
    if address % align == 0 {
        buf.truncate(size);
        return (buf, 0);
    }
    let padding = ((address & !(align - 1)).checked_add(align).unwrap())
        .checked_sub(address)
        .unwrap();
    assert!(padding <= 7, "padding of {} is bigger than 7", padding);
    assert!(
        padding <= extra,
        "padding of {} is bigger than extra {} bytes",
        padding,
        extra
    );
    buf.truncate(size + padding);
    assert_eq!(size + padding, buf.len());
    assert_eq!(
        0,
        buf[padding..].as_ptr().as_usize() % align,
        "expected end of initial padding to be aligned to {}",
        align,
    );
    (buf, padding)
}

/// Reads a NUL terminated label starting at the beginning of the given slice.
///
/// If a NUL terminated label could not be found, then an error is returned.
/// Similarly, if a label is found but doesn't match the expected label, then
/// an error is returned.
///
/// Upon success, the total number of bytes read (including padding bytes) is
/// returned.
pub(crate) fn read_label(
    slice: &[u8],
    expected_label: &'static str,
) -> Result<usize, DeserializeError> {
    // Set an upper bound on how many bytes we scan for a NUL. Since no label
    // in this crate is longer than 256 bytes, if we can't find one within that
    // range, then we have corrupted data.
    let first_nul =
        slice[..cmp::min(slice.len(), 256)].iter().position(|&b| b == 0);
    let first_nul = match first_nul {
        Some(first_nul) => first_nul,
        None => {
            return Err(DeserializeError::generic(
                "could not find NUL terminated label \
                 at start of serialized object",
            ));
        }
    };
    let len = first_nul + padding_len(first_nul);
    if slice.len() < len {
        return Err(DeserializeError::generic(
            "could not find properly sized label at start of serialized object"
        ));
    }
    if expected_label.as_bytes() != &slice[..first_nul] {
        return Err(DeserializeError::label_mismatch(expected_label));
    }
    Ok(len)
}

/// Writes the given label to the buffer as a NUL terminated string. The label
/// given must not contain NUL, otherwise this will panic. Similarly, the label
/// must not be longer than 255 bytes, otherwise this will panic.
///
/// Additional NUL bytes are written as necessary to ensure that the number of
/// bytes written is always a multiple of 4.
///
/// Upon success, the total number of bytes written (including padding) is
/// returned.
pub(crate) fn write_label(
    label: &str,
    dst: &mut [u8],
) -> Result<usize, SerializeError> {
    let nwrite = write_label_len(label);
    if dst.len() < nwrite {
        return Err(SerializeError::buffer_too_small("label"));
    }
    dst[..label.len()].copy_from_slice(label.as_bytes());
    for i in 0..(nwrite - label.len()) {
        dst[label.len() + i] = 0;
    }
    assert_eq!(nwrite % 4, 0);
    Ok(nwrite)
}

/// Returns the total number of bytes (including padding) that would be written
/// for the given label. This panics if the given label contains a NUL byte or
/// is longer than 255 bytes. (The size restriction exists so that searching
/// for a label during deserialization can be done in small bounded space.)
pub(crate) fn write_label_len(label: &str) -> usize {
    if label.len() > 255 {
        panic!("label must not be longer than 255 bytes");
    }
    if label.as_bytes().iter().position(|&b| b == 0).is_some() {
        panic!("label must not contain NUL bytes");
    }
    let label_len = label.len() + 1; // +1 for the NUL terminator
    label_len + padding_len(label_len)
}

/// Reads the endianness check from the beginning of the given slice and
/// confirms that the endianness of the serialized object matches the expected
/// endianness. If the slice is too small or if the endianness check fails,
/// this returns an error.
///
/// Upon success, the total number of bytes read is returned.
pub(crate) fn read_endianness_check(
    slice: &[u8],
) -> Result<usize, DeserializeError> {
    let (n, nr) = try_read_u32(slice, "endianness check")?;
    assert_eq!(nr, write_endianness_check_len());
    if n != 0xFEFF {
        return Err(DeserializeError::endian_mismatch(0xFEFF, n));
    }
    Ok(nr)
}

/// Writes 0xFEFF as an integer using the given endianness.
///
/// This is useful for writing into the header of a serialized object. It can
/// be read during deserialization as a sanity check to ensure the proper
/// endianness is used.
///
/// Upon success, the total number of bytes written is returned.
pub(crate) fn write_endianness_check<E: Endian>(
    dst: &mut [u8],
) -> Result<usize, SerializeError> {
    let nwrite = write_endianness_check_len();
    if dst.len() < nwrite {
        return Err(SerializeError::buffer_too_small("endianness check"));
    }
    E::write_u32(0xFEFF, dst);
    Ok(nwrite)
}

/// Returns the number of bytes written by the endianness check.
pub(crate) fn write_endianness_check_len() -> usize {
    size_of::<u32>()
}

/// Reads a version number from the beginning of the given slice and confirms
/// that is matches the expected version number given. If the slice is too
/// small or if the version numbers aren't equivalent, this returns an error.
///
/// Upon success, the total number of bytes read is returned.
///
/// N.B. Currently, we require that the version number is exactly equivalent.
/// In the future, if we bump the version number without a semver bump, then
/// we'll need to relax this a bit and support older versions.
pub(crate) fn read_version(
    slice: &[u8],
    expected_version: u32,
) -> Result<usize, DeserializeError> {
    let (n, nr) = try_read_u32(slice, "version")?;
    assert_eq!(nr, write_version_len());
    if n != expected_version {
        return Err(DeserializeError::version_mismatch(expected_version, n));
    }
    Ok(nr)
}

/// Writes the given version number to the beginning of the given slice.
///
/// This is useful for writing into the header of a serialized object. It can
/// be read during deserialization as a sanity check to ensure that the library
/// code supports the format of the serialized object.
///
/// Upon success, the total number of bytes written is returned.
pub(crate) fn write_version<E: Endian>(
    version: u32,
    dst: &mut [u8],
) -> Result<usize, SerializeError> {
    let nwrite = write_version_len();
    if dst.len() < nwrite {
        return Err(SerializeError::buffer_too_small("version number"));
    }
    E::write_u32(version, dst);
    Ok(nwrite)
}

/// Returns the number of bytes written by writing the version number.
pub(crate) fn write_version_len() -> usize {
    size_of::<u32>()
}

/// Reads a pattern ID from the given slice. If the slice has insufficient
/// length, then this panics. If the deserialized integer exceeds the pattern
/// ID limit for the current target, then this returns an error.
///
/// Upon success, this also returns the number of bytes read.
pub(crate) fn read_pattern_id(
    slice: &[u8],
    what: &'static str,
) -> Result<(PatternID, usize), DeserializeError> {
    let bytes: [u8; PatternID::SIZE] =
        slice[..PatternID::SIZE].try_into().unwrap();
    let pid = PatternID::from_ne_bytes(bytes)
        .map_err(|err| DeserializeError::pattern_id_error(err, what))?;
    Ok((pid, PatternID::SIZE))
}

/// Reads a pattern ID from the given slice. If the slice has insufficient
/// length, then this panics. Otherwise, the deserialized integer is assumed
/// to be a valid pattern ID.
///
/// This also returns the number of bytes read.
pub(crate) fn read_pattern_id_unchecked(slice: &[u8]) -> (PatternID, usize) {
    let pid = PatternID::from_ne_bytes_unchecked(
        slice[..PatternID::SIZE].try_into().unwrap(),
    );
    (pid, PatternID::SIZE)
}

/// Write the given pattern ID to the beginning of the given slice of bytes
/// using the specified endianness. The given slice must have length at least
/// `PatternID::SIZE`, or else this panics. Upon success, the total number of
/// bytes written is returned.
pub(crate) fn write_pattern_id<E: Endian>(
    pid: PatternID,
    dst: &mut [u8],
) -> usize {
    E::write_u32(pid.as_u32(), dst);
    PatternID::SIZE
}

/// Attempts to read a state ID from the given slice. If the slice has an
/// insufficient number of bytes or if the state ID exceeds the limit for
/// the current target, then this returns an error.
///
/// Upon success, this also returns the number of bytes read.
pub(crate) fn try_read_state_id(
    slice: &[u8],
    what: &'static str,
) -> Result<(StateID, usize), DeserializeError> {
    if slice.len() < StateID::SIZE {
        return Err(DeserializeError::buffer_too_small(what));
    }
    read_state_id(slice, what)
}

/// Reads a state ID from the given slice. If the slice has insufficient
/// length, then this panics. If the deserialized integer exceeds the state ID
/// limit for the current target, then this returns an error.
///
/// Upon success, this also returns the number of bytes read.
pub(crate) fn read_state_id(
    slice: &[u8],
    what: &'static str,
) -> Result<(StateID, usize), DeserializeError> {
    let bytes: [u8; StateID::SIZE] =
        slice[..StateID::SIZE].try_into().unwrap();
    let sid = StateID::from_ne_bytes(bytes)
        .map_err(|err| DeserializeError::state_id_error(err, what))?;
    Ok((sid, StateID::SIZE))
}

/// Reads a state ID from the given slice. If the slice has insufficient
/// length, then this panics. Otherwise, the deserialized integer is assumed
/// to be a valid state ID.
///
/// This also returns the number of bytes read.
pub(crate) fn read_state_id_unchecked(slice: &[u8]) -> (StateID, usize) {
    let sid = StateID::from_ne_bytes_unchecked(
        slice[..StateID::SIZE].try_into().unwrap(),
    );
    (sid, StateID::SIZE)
}

/// Write the given state ID to the beginning of the given slice of bytes
/// using the specified endianness. The given slice must have length at least
/// `StateID::SIZE`, or else this panics. Upon success, the total number of
/// bytes written is returned.
pub(crate) fn write_state_id<E: Endian>(
    sid: StateID,
    dst: &mut [u8],
) -> usize {
    E::write_u32(sid.as_u32(), dst);
    StateID::SIZE
}

/// Try to read a u16 as a usize from the beginning of the given slice in
/// native endian format. If the slice has fewer than 2 bytes or if the
/// deserialized number cannot be represented by usize, then this returns an
/// error. The error message will include the `what` description of what is
/// being deserialized, for better error messages. `what` should be a noun in
/// singular form.
///
/// Upon success, this also returns the number of bytes read.
pub(crate) fn try_read_u16_as_usize(
    slice: &[u8],
    what: &'static str,
) -> Result<(usize, usize), DeserializeError> {
    try_read_u16(slice, what).and_then(|(n, nr)| {
        usize::try_from(n)
            .map(|n| (n, nr))
            .map_err(|_| DeserializeError::invalid_usize(what))
    })
}

/// Try to read a u32 as a usize from the beginning of the given slice in
/// native endian format. If the slice has fewer than 4 bytes or if the
/// deserialized number cannot be represented by usize, then this returns an
/// error. The error message will include the `what` description of what is
/// being deserialized, for better error messages. `what` should be a noun in
/// singular form.
///
/// Upon success, this also returns the number of bytes read.
pub(crate) fn try_read_u32_as_usize(
    slice: &[u8],
    what: &'static str,
) -> Result<(usize, usize), DeserializeError> {
    try_read_u32(slice, what).and_then(|(n, nr)| {
        usize::try_from(n)
            .map(|n| (n, nr))
            .map_err(|_| DeserializeError::invalid_usize(what))
    })
}

/// Try to read a u16 from the beginning of the given slice in native endian
/// format. If the slice has fewer than 2 bytes, then this returns an error.
/// The error message will include the `what` description of what is being
/// deserialized, for better error messages. `what` should be a noun in
/// singular form.
///
/// Upon success, this also returns the number of bytes read.
pub(crate) fn try_read_u16(
    slice: &[u8],
    what: &'static str,
) -> Result<(u16, usize), DeserializeError> {
    check_slice_len(slice, size_of::<u16>(), what)?;
    Ok((read_u16(slice), size_of::<u16>()))
}

/// Try to read a u32 from the beginning of the given slice in native endian
/// format. If the slice has fewer than 4 bytes, then this returns an error.
/// The error message will include the `what` description of what is being
/// deserialized, for better error messages. `what` should be a noun in
/// singular form.
///
/// Upon success, this also returns the number of bytes read.
pub(crate) fn try_read_u32(
    slice: &[u8],
    what: &'static str,
) -> Result<(u32, usize), DeserializeError> {
    check_slice_len(slice, size_of::<u32>(), what)?;
    Ok((read_u32(slice), size_of::<u32>()))
}

/// Try to read a u128 from the beginning of the given slice in native endian
/// format. If the slice has fewer than 16 bytes, then this returns an error.
/// The error message will include the `what` description of what is being
/// deserialized, for better error messages. `what` should be a noun in
/// singular form.
///
/// Upon success, this also returns the number of bytes read.
pub(crate) fn try_read_u128(
    slice: &[u8],
    what: &'static str,
) -> Result<(u128, usize), DeserializeError> {
    check_slice_len(slice, size_of::<u128>(), what)?;
    Ok((read_u128(slice), size_of::<u128>()))
}

/// Read a u16 from the beginning of the given slice in native endian format.
/// If the slice has fewer than 2 bytes, then this panics.
///
/// Marked as inline to speed up sparse searching which decodes integers from
/// its automaton at search time.
#[cfg_attr(feature = "perf-inline", inline(always))]
pub(crate) fn read_u16(slice: &[u8]) -> u16 {
    let bytes: [u8; 2] = slice[..size_of::<u16>()].try_into().unwrap();
    u16::from_ne_bytes(bytes)
}

/// Read a u32 from the beginning of the given slice in native endian format.
/// If the slice has fewer than 4 bytes, then this panics.
///
/// Marked as inline to speed up sparse searching which decodes integers from
/// its automaton at search time.
#[cfg_attr(feature = "perf-inline", inline(always))]
pub(crate) fn read_u32(slice: &[u8]) -> u32 {
    let bytes: [u8; 4] = slice[..size_of::<u32>()].try_into().unwrap();
    u32::from_ne_bytes(bytes)
}

/// Read a u128 from the beginning of the given slice in native endian format.
/// If the slice has fewer than 16 bytes, then this panics.
pub(crate) fn read_u128(slice: &[u8]) -> u128 {
    let bytes: [u8; 16] = slice[..size_of::<u128>()].try_into().unwrap();
    u128::from_ne_bytes(bytes)
}

/// Checks that the given slice has some minimal length. If it's smaller than
/// the bound given, then a "buffer too small" error is returned with `what`
/// describing what the buffer represents.
pub(crate) fn check_slice_len<T>(
    slice: &[T],
    at_least_len: usize,
    what: &'static str,
) -> Result<(), DeserializeError> {
    if slice.len() < at_least_len {
        return Err(DeserializeError::buffer_too_small(what));
    }
    Ok(())
}

/// Multiply the given numbers, and on overflow, return an error that includes
/// 'what' in the error message.
///
/// This is useful when doing arithmetic with untrusted data.
pub(crate) fn mul(
    a: usize,
    b: usize,
    what: &'static str,
) -> Result<usize, DeserializeError> {
    match a.checked_mul(b) {
        Some(c) => Ok(c),
        None => Err(DeserializeError::arithmetic_overflow(what)),
    }
}

/// Add the given numbers, and on overflow, return an error that includes
/// 'what' in the error message.
///
/// This is useful when doing arithmetic with untrusted data.
pub(crate) fn add(
    a: usize,
    b: usize,
    what: &'static str,
) -> Result<usize, DeserializeError> {
    match a.checked_add(b) {
        Some(c) => Ok(c),
        None => Err(DeserializeError::arithmetic_overflow(what)),
    }
}

/// Shift `a` left by `b`, and on overflow, return an error that includes
/// 'what' in the error message.
///
/// This is useful when doing arithmetic with untrusted data.
pub(crate) fn shl(
    a: usize,
    b: usize,
    what: &'static str,
) -> Result<usize, DeserializeError> {
    let amount = u32::try_from(b)
        .map_err(|_| DeserializeError::arithmetic_overflow(what))?;
    match a.checked_shl(amount) {
        Some(c) => Ok(c),
        None => Err(DeserializeError::arithmetic_overflow(what)),
    }
}

/// Returns the number of additional bytes required to add to the given length
/// in order to make the total length a multiple of 4. The return value is
/// always less than 4.
pub(crate) fn padding_len(non_padding_len: usize) -> usize {
    (4 - (non_padding_len & 0b11)) & 0b11
}

/// A simple trait for writing code generic over endianness.
///
/// This is similar to what byteorder provides, but we only need a very small
/// subset.
pub(crate) trait Endian {
    /// Writes a u16 to the given destination buffer in a particular
    /// endianness. If the destination buffer has a length smaller than 2, then
    /// this panics.
    fn write_u16(n: u16, dst: &mut [u8]);

    /// Writes a u32 to the given destination buffer in a particular
    /// endianness. If the destination buffer has a length smaller than 4, then
    /// this panics.
    fn write_u32(n: u32, dst: &mut [u8]);

    /// Writes a u128 to the given destination buffer in a particular
    /// endianness. If the destination buffer has a length smaller than 16,
    /// then this panics.
    fn write_u128(n: u128, dst: &mut [u8]);
}

/// Little endian writing.
pub(crate) enum LE {}
/// Big endian writing.
pub(crate) enum BE {}

#[cfg(target_endian = "little")]
pub(crate) type NE = LE;
#[cfg(target_endian = "big")]
pub(crate) type NE = BE;

impl Endian for LE {
    fn write_u16(n: u16, dst: &mut [u8]) {
        dst[..2].copy_from_slice(&n.to_le_bytes());
    }

    fn write_u32(n: u32, dst: &mut [u8]) {
        dst[..4].copy_from_slice(&n.to_le_bytes());
    }

    fn write_u128(n: u128, dst: &mut [u8]) {
        dst[..16].copy_from_slice(&n.to_le_bytes());
    }
}

impl Endian for BE {
    fn write_u16(n: u16, dst: &mut [u8]) {
        dst[..2].copy_from_slice(&n.to_be_bytes());
    }

    fn write_u32(n: u32, dst: &mut [u8]) {
        dst[..4].copy_from_slice(&n.to_be_bytes());
    }

    fn write_u128(n: u128, dst: &mut [u8]) {
        dst[..16].copy_from_slice(&n.to_be_bytes());
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        let mut buf = [0; 1024];

        let nwrite = write_label("fooba", &mut buf).unwrap();
        assert_eq!(nwrite, 8);
        assert_eq!(&buf[..nwrite], b"fooba\x00\x00\x00");

        let nread = read_label(&buf, "fooba").unwrap();
        assert_eq!(nread, 8);
    }

    #[test]
    #[should_panic]
    fn bad_label_interior_nul() {
        // interior NULs are not allowed
        write_label("foo\x00bar", &mut [0; 1024]).unwrap();
    }

    #[test]
    fn bad_label_almost_too_long() {
        // ok
        write_label(&"z".repeat(255), &mut [0; 1024]).unwrap();
    }

    #[test]
    #[should_panic]
    fn bad_label_too_long() {
        // labels longer than 255 bytes are banned
        write_label(&"z".repeat(256), &mut [0; 1024]).unwrap();
    }

    #[test]
    fn padding() {
        assert_eq!(0, padding_len(8));
        assert_eq!(3, padding_len(9));
        assert_eq!(2, padding_len(10));
        assert_eq!(1, padding_len(11));
        assert_eq!(0, padding_len(12));
        assert_eq!(3, padding_len(13));
        assert_eq!(2, padding_len(14));
        assert_eq!(1, padding_len(15));
        assert_eq!(0, padding_len(16));
    }
}
