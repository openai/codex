use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecodedLength(u64);

#[cfg(any(feature = "http1", feature = "http2"))]
impl From<Option<u64>> for DecodedLength {
    fn from(len: Option<u64>) -> Self {
        len.and_then(|len| {
            // If the length is u64::MAX, oh well, just reported chunked.
            Self::checked_new(len).ok()
        })
        .unwrap_or(DecodedLength::CHUNKED)
    }
}

#[cfg(any(feature = "http1", feature = "http2", test))]
const MAX_LEN: u64 = u64::MAX - 2;

impl DecodedLength {
    pub(crate) const CLOSE_DELIMITED: DecodedLength = DecodedLength(u64::MAX);
    pub(crate) const CHUNKED: DecodedLength = DecodedLength(u64::MAX - 1);
    pub(crate) const ZERO: DecodedLength = DecodedLength(0);

    #[cfg(test)]
    pub(crate) fn new(len: u64) -> Self {
        debug_assert!(len <= MAX_LEN);
        DecodedLength(len)
    }

    /// Takes the length as a content-length without other checks.
    ///
    /// Should only be called if previously confirmed this isn't
    /// CLOSE_DELIMITED or CHUNKED.
    #[inline]
    #[cfg(all(any(feature = "client", feature = "server"), feature = "http1"))]
    pub(crate) fn danger_len(self) -> u64 {
        debug_assert!(self.0 < Self::CHUNKED.0);
        self.0
    }

    /// Converts to an Option<u64> representing a Known or Unknown length.
    #[cfg(all(
        any(feature = "http1", feature = "http2"),
        any(feature = "client", feature = "server")
    ))]
    pub(crate) fn into_opt(self) -> Option<u64> {
        match self {
            DecodedLength::CHUNKED | DecodedLength::CLOSE_DELIMITED => None,
            DecodedLength(known) => Some(known),
        }
    }

    /// Checks the `u64` is within the maximum allowed for content-length.
    #[cfg(any(feature = "http1", feature = "http2"))]
    pub(crate) fn checked_new(len: u64) -> Result<Self, crate::error::Parse> {
        if len <= MAX_LEN {
            Ok(DecodedLength(len))
        } else {
            warn!("content-length bigger than maximum: {} > {}", len, MAX_LEN);
            Err(crate::error::Parse::TooLarge)
        }
    }

    #[cfg(all(
        any(feature = "http1", feature = "http2"),
        any(feature = "client", feature = "server")
    ))]
    pub(crate) fn sub_if(&mut self, amt: u64) {
        match *self {
            DecodedLength::CHUNKED | DecodedLength::CLOSE_DELIMITED => (),
            DecodedLength(ref mut known) => {
                *known -= amt;
            }
        }
    }

    /// Returns whether this represents an exact length.
    ///
    /// This includes 0, which of course is an exact known length.
    ///
    /// It would return false if "chunked" or otherwise size-unknown.
    #[cfg(all(any(feature = "client", feature = "server"), feature = "http2"))]
    pub(crate) fn is_exact(&self) -> bool {
        self.0 <= MAX_LEN
    }
}

impl fmt::Debug for DecodedLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DecodedLength::CLOSE_DELIMITED => f.write_str("CLOSE_DELIMITED"),
            DecodedLength::CHUNKED => f.write_str("CHUNKED"),
            DecodedLength(n) => f.debug_tuple("DecodedLength").field(&n).finish(),
        }
    }
}

impl fmt::Display for DecodedLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DecodedLength::CLOSE_DELIMITED => f.write_str("close-delimited"),
            DecodedLength::CHUNKED => f.write_str("chunked encoding"),
            DecodedLength::ZERO => f.write_str("empty"),
            DecodedLength(n) => write!(f, "content-length ({} bytes)", n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_if_known() {
        let mut len = DecodedLength::new(30);
        len.sub_if(20);

        assert_eq!(len.0, 10);
    }

    #[test]
    fn sub_if_chunked() {
        let mut len = DecodedLength::CHUNKED;
        len.sub_if(20);

        assert_eq!(len, DecodedLength::CHUNKED);
    }
}
