use std::array;

use http::header::{self, HeaderName, HeaderValue};

use super::preflight_request_headers;

/// Holds configuration for how to set the [`Vary`][mdn] header.
///
/// See [`CorsLayer::vary`] for more details.
///
/// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Vary
/// [`CorsLayer::vary`]: super::CorsLayer::vary
#[derive(Clone, Debug)]
pub struct Vary(Vec<HeaderValue>);

impl Vary {
    /// Set the list of header names to return as vary header values
    ///
    /// See [`CorsLayer::vary`] for more details.
    ///
    /// [`CorsLayer::vary`]: super::CorsLayer::vary
    pub fn list<I>(headers: I) -> Self
    where
        I: IntoIterator<Item = HeaderName>,
    {
        Self(headers.into_iter().map(Into::into).collect())
    }

    pub(super) fn to_header(&self) -> Option<(HeaderName, HeaderValue)> {
        let values = &self.0;
        let mut res = values.first()?.as_bytes().to_owned();
        for val in &values[1..] {
            res.extend_from_slice(b", ");
            res.extend_from_slice(val.as_bytes());
        }

        let header_val = HeaderValue::from_bytes(&res)
            .expect("comma-separated list of HeaderValues is always a valid HeaderValue");
        Some((header::VARY, header_val))
    }
}

impl Default for Vary {
    fn default() -> Self {
        Self::list(preflight_request_headers())
    }
}

impl<const N: usize> From<[HeaderName; N]> for Vary {
    fn from(arr: [HeaderName; N]) -> Self {
        #[allow(deprecated)] // Can be changed when MSRV >= 1.53
        Self::list(array::IntoIter::new(arr))
    }
}

impl From<Vec<HeaderName>> for Vary {
    fn from(vec: Vec<HeaderName>) -> Self {
        Self::list(vec)
    }
}
