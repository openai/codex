use bytes::Bytes;

/// A reason phrase in an HTTP/1 response.
///
/// # Clients
///
/// For clients, a `ReasonPhrase` will be present in the extensions of the `http::Response` returned
/// for a request if the reason phrase is different from the canonical reason phrase for the
/// response's status code. For example, if a server returns `HTTP/1.1 200 Awesome`, the
/// `ReasonPhrase` will be present and contain `Awesome`, but if a server returns `HTTP/1.1 200 OK`,
/// the response will not contain a `ReasonPhrase`.
///
/// ```no_run
/// # #[cfg(all(feature = "tcp", feature = "client", feature = "http1"))]
/// # async fn fake_fetch() -> hyper::Result<()> {
/// use hyper::{Client, Uri};
/// use hyper::ext::ReasonPhrase;
///
/// let res = Client::new().get(Uri::from_static("http://example.com/non_canonical_reason")).await?;
///
/// // Print out the non-canonical reason phrase, if it has one...
/// if let Some(reason) = res.extensions().get::<ReasonPhrase>() {
///     println!("non-canonical reason: {}", std::str::from_utf8(reason.as_bytes()).unwrap());
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Servers
///
/// When a `ReasonPhrase` is present in the extensions of the `http::Response` written by a server,
/// its contents will be written in place of the canonical reason phrase when responding via HTTP/1.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReasonPhrase(Bytes);

impl ReasonPhrase {
    /// Gets the reason phrase as bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Converts a static byte slice to a reason phrase.
    pub const fn from_static(reason: &'static [u8]) -> Self {
        // TODO: this can be made const once MSRV is >= 1.57.0
        if find_invalid_byte(reason).is_some() {
            panic!("invalid byte in static reason phrase");
        }
        Self(Bytes::from_static(reason))
    }

    // Not public on purpose.
    /// Converts a `Bytes` directly into a `ReasonPhrase` without validating.
    ///
    /// Use with care; invalid bytes in a reason phrase can cause serious security problems if
    /// emitted in a response.
    #[cfg(feature = "client")]
    pub(crate) fn from_bytes_unchecked(reason: Bytes) -> Self {
        Self(reason)
    }
}

impl TryFrom<&[u8]> for ReasonPhrase {
    type Error = InvalidReasonPhrase;

    fn try_from(reason: &[u8]) -> Result<Self, Self::Error> {
        if let Some(bad_byte) = find_invalid_byte(reason) {
            Err(InvalidReasonPhrase { bad_byte })
        } else {
            Ok(Self(Bytes::copy_from_slice(reason)))
        }
    }
}

impl TryFrom<Vec<u8>> for ReasonPhrase {
    type Error = InvalidReasonPhrase;

    fn try_from(reason: Vec<u8>) -> Result<Self, Self::Error> {
        if let Some(bad_byte) = find_invalid_byte(&reason) {
            Err(InvalidReasonPhrase { bad_byte })
        } else {
            Ok(Self(Bytes::from(reason)))
        }
    }
}

impl TryFrom<String> for ReasonPhrase {
    type Error = InvalidReasonPhrase;

    fn try_from(reason: String) -> Result<Self, Self::Error> {
        if let Some(bad_byte) = find_invalid_byte(reason.as_bytes()) {
            Err(InvalidReasonPhrase { bad_byte })
        } else {
            Ok(Self(Bytes::from(reason)))
        }
    }
}

impl TryFrom<Bytes> for ReasonPhrase {
    type Error = InvalidReasonPhrase;

    fn try_from(reason: Bytes) -> Result<Self, Self::Error> {
        if let Some(bad_byte) = find_invalid_byte(&reason) {
            Err(InvalidReasonPhrase { bad_byte })
        } else {
            Ok(Self(reason))
        }
    }
}

impl From<ReasonPhrase> for Bytes {
    fn from(reason: ReasonPhrase) -> Self {
        reason.0
    }
}

impl AsRef<[u8]> for ReasonPhrase {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Error indicating an invalid byte when constructing a `ReasonPhrase`.
///
/// See [the spec][spec] for details on allowed bytes.
///
/// [spec]: https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#rfc.section.4.p.7
#[derive(Debug)]
pub struct InvalidReasonPhrase {
    bad_byte: u8,
}

impl std::fmt::Display for InvalidReasonPhrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid byte in reason phrase: {}", self.bad_byte)
    }
}

impl std::error::Error for InvalidReasonPhrase {}

const fn is_valid_byte(b: u8) -> bool {
    // See https://www.rfc-editor.org/rfc/rfc5234.html#appendix-B.1
    const fn is_vchar(b: u8) -> bool {
        0x21 <= b && b <= 0x7E
    }

    // See https://httpwg.org/http-core/draft-ietf-httpbis-semantics-latest.html#fields.values
    //
    // The 0xFF comparison is technically redundant, but it matches the text of the spec more
    // clearly and will be optimized away.
    #[allow(unused_comparisons, clippy::absurd_extreme_comparisons)]
    const fn is_obs_text(b: u8) -> bool {
        0x80 <= b && b <= 0xFF
    }

    // See https://httpwg.org/http-core/draft-ietf-httpbis-messaging-latest.html#rfc.section.4.p.7
    b == b'\t' || b == b' ' || is_vchar(b) || is_obs_text(b)
}

const fn find_invalid_byte(bytes: &[u8]) -> Option<u8> {
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if !is_valid_byte(b) {
            return Some(b);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_valid() {
        const PHRASE: &[u8] = b"OK";
        assert_eq!(ReasonPhrase::from_static(PHRASE).as_bytes(), PHRASE);
        assert_eq!(ReasonPhrase::try_from(PHRASE).unwrap().as_bytes(), PHRASE);
    }

    #[test]
    fn empty_valid() {
        const PHRASE: &[u8] = b"";
        assert_eq!(ReasonPhrase::from_static(PHRASE).as_bytes(), PHRASE);
        assert_eq!(ReasonPhrase::try_from(PHRASE).unwrap().as_bytes(), PHRASE);
    }

    #[test]
    fn obs_text_valid() {
        const PHRASE: &[u8] = b"hyp\xe9r";
        assert_eq!(ReasonPhrase::from_static(PHRASE).as_bytes(), PHRASE);
        assert_eq!(ReasonPhrase::try_from(PHRASE).unwrap().as_bytes(), PHRASE);
    }

    const NEWLINE_PHRASE: &[u8] = b"hyp\ner";

    #[test]
    #[should_panic]
    fn newline_invalid_panic() {
        ReasonPhrase::from_static(NEWLINE_PHRASE);
    }

    #[test]
    fn newline_invalid_err() {
        assert!(ReasonPhrase::try_from(NEWLINE_PHRASE).is_err());
    }

    const CR_PHRASE: &[u8] = b"hyp\rer";

    #[test]
    #[should_panic]
    fn cr_invalid_panic() {
        ReasonPhrase::from_static(CR_PHRASE);
    }

    #[test]
    fn cr_invalid_err() {
        assert!(ReasonPhrase::try_from(CR_PHRASE).is_err());
    }
}
