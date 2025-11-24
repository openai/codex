//! Background readiness types

opaque_future! {
    /// Response future from [`SpawnReady`] services.
    ///
    /// [`SpawnReady`]: crate::spawn_ready::SpawnReady
    pub type ResponseFuture<F, E> = futures_util::future::MapErr<F, fn(E) -> crate::BoxError>;
}
