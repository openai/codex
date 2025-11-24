use core::mem::size_of;

use crate::util::wire::{self, DeserializeError, Endian, SerializeError};

/// The kind of anchored starting configurations to support in a DFA.
///
/// Fully compiled DFAs need to be explicitly configured as to which anchored
/// starting configurations to support. The reason for not just supporting
/// everything unconditionally is that it can use more resources (such as
/// memory and build time). The downside of this is that if you try to execute
/// a search using an [`Anchored`](crate::Anchored) mode that is not supported
/// by the DFA, then the search will return an error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartKind {
    /// Support both anchored and unanchored searches.
    Both,
    /// Support only unanchored searches. Requesting an anchored search will
    /// panic.
    ///
    /// Note that even if an unanchored search is requested, the pattern itself
    /// may still be anchored. For example, `^abc` will only match `abc` at the
    /// start of a haystack. This will remain true, even if the regex engine
    /// only supported unanchored searches.
    Unanchored,
    /// Support only anchored searches. Requesting an unanchored search will
    /// panic.
    Anchored,
}

impl StartKind {
    pub(crate) fn from_bytes(
        slice: &[u8],
    ) -> Result<(StartKind, usize), DeserializeError> {
        wire::check_slice_len(slice, size_of::<u32>(), "start kind bytes")?;
        let (n, nr) = wire::try_read_u32(slice, "start kind integer")?;
        match n {
            0 => Ok((StartKind::Both, nr)),
            1 => Ok((StartKind::Unanchored, nr)),
            2 => Ok((StartKind::Anchored, nr)),
            _ => Err(DeserializeError::generic("unrecognized start kind")),
        }
    }

    pub(crate) fn write_to<E: Endian>(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("start kind"));
        }
        let n = match *self {
            StartKind::Both => 0,
            StartKind::Unanchored => 1,
            StartKind::Anchored => 2,
        };
        E::write_u32(n, dst);
        Ok(nwrite)
    }

    pub(crate) fn write_to_len(&self) -> usize {
        size_of::<u32>()
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn has_unanchored(&self) -> bool {
        matches!(*self, StartKind::Both | StartKind::Unanchored)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn has_anchored(&self) -> bool {
        matches!(*self, StartKind::Both | StartKind::Anchored)
    }
}
