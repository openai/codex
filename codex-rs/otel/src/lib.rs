pub mod config;

#[cfg(feature = "otel")]
mod file_exporter;

#[cfg(feature = "otel")]
pub mod otel_provider;

#[cfg(not(feature = "otel"))]
mod imp {
    use reqwest::header::HeaderMap;
    use tracing::Span;

    pub struct OtelProvider;

    impl OtelProvider {
        pub fn from(_settings: &crate::config::OtelSettings) -> Option<Self> {
            None
        }

        pub fn headers(_span: &Span) -> HeaderMap {
            HeaderMap::new()
        }
    }
}

#[cfg(not(feature = "otel"))]
pub use imp::OtelProvider;
