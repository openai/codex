/// Spawns tasks to drive its inner service to readiness.
#[derive(Clone, Debug, Default)]
pub struct SpawnReadyLayer(());

impl SpawnReadyLayer {
    /// Builds a [`SpawnReadyLayer`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl<S> tower_layer::Layer<S> for SpawnReadyLayer {
    type Service = super::SpawnReady<S>;

    fn layer(&self, service: S) -> Self::Service {
        super::SpawnReady::new(service)
    }
}
