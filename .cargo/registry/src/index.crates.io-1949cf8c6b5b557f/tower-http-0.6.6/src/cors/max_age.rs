use std::{fmt, sync::Arc, time::Duration};

use http::{
    header::{self, HeaderName, HeaderValue},
    request::Parts as RequestParts,
};

/// Holds configuration for how to set the [`Access-Control-Max-Age`][mdn] header.
///
/// See [`CorsLayer::max_age`][super::CorsLayer::max_age] for more details.
///
/// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Max-Age
#[derive(Clone, Default)]
#[must_use]
pub struct MaxAge(MaxAgeInner);

impl MaxAge {
    /// Set a static max-age value
    ///
    /// See [`CorsLayer::max_age`][super::CorsLayer::max_age] for more details.
    pub fn exact(max_age: Duration) -> Self {
        Self(MaxAgeInner::Exact(Some(max_age.as_secs().into())))
    }

    /// Set the max-age based on the preflight request parts
    ///
    /// See [`CorsLayer::max_age`][super::CorsLayer::max_age] for more details.
    pub fn dynamic<F>(f: F) -> Self
    where
        F: Fn(&HeaderValue, &RequestParts) -> Duration + Send + Sync + 'static,
    {
        Self(MaxAgeInner::Fn(Arc::new(f)))
    }

    pub(super) fn to_header(
        &self,
        origin: Option<&HeaderValue>,
        parts: &RequestParts,
    ) -> Option<(HeaderName, HeaderValue)> {
        let max_age = match &self.0 {
            MaxAgeInner::Exact(v) => v.clone()?,
            MaxAgeInner::Fn(c) => c(origin?, parts).as_secs().into(),
        };

        Some((header::ACCESS_CONTROL_MAX_AGE, max_age))
    }
}

impl fmt::Debug for MaxAge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            MaxAgeInner::Exact(inner) => f.debug_tuple("Exact").field(inner).finish(),
            MaxAgeInner::Fn(_) => f.debug_tuple("Fn").finish(),
        }
    }
}

impl From<Duration> for MaxAge {
    fn from(max_age: Duration) -> Self {
        Self::exact(max_age)
    }
}

#[derive(Clone)]
enum MaxAgeInner {
    Exact(Option<HeaderValue>),
    Fn(Arc<dyn for<'a> Fn(&'a HeaderValue, &'a RequestParts) -> Duration + Send + Sync + 'static>),
}

impl Default for MaxAgeInner {
    fn default() -> Self {
        Self::Exact(None)
    }
}
