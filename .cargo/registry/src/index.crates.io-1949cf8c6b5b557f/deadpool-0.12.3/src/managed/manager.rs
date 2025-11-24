use std::future::Future;

use crate::managed::{Metrics, RecycleError};

/// Manager responsible for creating new [`super::Object`]s or recycling existing ones.
pub trait Manager: Sync + Send {
    /// Type of [`super::Object`]s that this [`Manager`] creates and recycles.
    type Type: Send;
    /// Error that this [`Manager`] can return when creating and/or recycling
    /// [`super::Object`]s.
    type Error: Send;

    /// Creates a new instance of [`Manager::Type`].
    fn create(&self) -> impl Future<Output = Result<Self::Type, Self::Error>> + Send;

    /// Tries to recycle an instance of [`Manager::Type`].
    ///
    /// # Errors
    ///
    /// Returns [`Manager::Error`] if the instance couldn't be recycled.
    fn recycle(
        &self,
        obj: &mut Self::Type,
        metrics: &Metrics,
    ) -> impl Future<Output = RecycleResult<Self::Error>> + Send;

    /// Detaches an instance of [`Manager::Type`] from this [`Manager`].
    ///
    /// This method is called when using the [`super::Object::take()`] method for
    /// removing an [`super::Object`] from a [`super::Pool`]. If the [`Manager`] doesn't hold
    /// any references to the handed out [`super::Object`]s then the default
    /// implementation can be used which does nothing.
    fn detach(&self, _obj: &mut Self::Type) {}
}

/// Result type of the [`Manager::recycle()`] method.
pub type RecycleResult<E> = Result<(), RecycleError<E>>;
