use crate::error_handling::HandleError;
#[cfg(feature = "tokio")]
use crate::extract::connect_info::IntoMakeServiceWithConnectInfo;
use crate::routing::IntoMakeService;
use tower_service::Service;

/// Extension trait that adds additional methods to any [`Service`].
pub trait ServiceExt<R>: Service<R> + Sized {
    /// Convert this service into a [`MakeService`], that is a [`Service`] whose
    /// response is another service.
    ///
    /// This is commonly used when applying middleware around an entire [`Router`]. See ["Rewriting
    /// request URI in middleware"] for more details.
    ///
    /// [`MakeService`]: tower::make::MakeService
    /// ["Rewriting request URI in middleware"]: crate::middleware#rewriting-request-uri-in-middleware
    /// [`Router`]: crate::Router
    fn into_make_service(self) -> IntoMakeService<Self>;

    /// Convert this service into a [`MakeService`], that will store `C`'s
    /// associated `ConnectInfo` in a request extension such that [`ConnectInfo`]
    /// can extract it.
    ///
    /// This enables extracting things like the client's remote address.
    /// This is commonly used when applying middleware around an entire [`Router`]. See ["Rewriting
    /// request URI in middleware"] for more details.
    ///
    /// [`MakeService`]: tower::make::MakeService
    /// ["Rewriting request URI in middleware"]: crate::middleware#rewriting-request-uri-in-middleware
    /// [`Router`]: crate::Router
    /// [`ConnectInfo`]: crate::extract::connect_info::ConnectInfo
    #[cfg(feature = "tokio")]
    fn into_make_service_with_connect_info<C>(self) -> IntoMakeServiceWithConnectInfo<Self, C>;

    /// Convert this service into a [`HandleError`], that will handle errors
    /// by converting them into responses.
    ///
    ///  See ["error handling model"] for more details.
    ///
    /// [`HandleError`]: crate::error_handling::HandleError
    /// ["error handling model"]: crate::error_handling#axums-error-handling-model
    fn handle_error<F, T>(self, f: F) -> HandleError<Self, F, T> {
        HandleError::new(self, f)
    }
}

impl<S, R> ServiceExt<R> for S
where
    S: Service<R> + Sized,
{
    fn into_make_service(self) -> IntoMakeService<Self> {
        IntoMakeService::new(self)
    }

    #[cfg(feature = "tokio")]
    fn into_make_service_with_connect_info<C>(self) -> IntoMakeServiceWithConnectInfo<Self, C> {
        IntoMakeServiceWithConnectInfo::new(self)
    }
}
