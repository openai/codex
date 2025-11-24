use super::Layer;
use std::fmt;

/// A no-op middleware.
///
/// When wrapping a [`Service`], the [`Identity`] layer returns the provided
/// service without modifying it.
///
/// [`Service`]: https://docs.rs/tower-service/latest/tower_service/trait.Service.html
#[derive(Default, Clone)]
pub struct Identity {
    _p: (),
}

impl Identity {
    /// Create a new [`Identity`] value
    pub const fn new() -> Identity {
        Identity { _p: () }
    }
}

/// Decorates a [`Service`], transforming either the request or the response.
///
/// [`Service`]: https://docs.rs/tower-service/latest/tower_service/trait.Service.html
impl<S> Layer<S> for Identity {
    type Service = S;

    fn layer(&self, inner: S) -> Self::Service {
        inner
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity").finish()
    }
}
