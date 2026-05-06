use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use http::HeaderValue;

pub(crate) const X_OAI_ATTESTATION_HEADER: &str = "x-oai-attestation";

type GenerateAttestationFuture = Pin<Box<dyn Future<Output = Option<String>> + Send>>;
type GenerateAttestationCallback = dyn Fn() -> GenerateAttestationFuture + Send + Sync + 'static;

/// Session-scoped source for just-in-time attestation header values.
///
/// Host integrations provide the opaque string expected by the upstream
/// `x-oai-attestation` header. Core validates only that it is legal as an HTTP
/// header value before forwarding it.
#[derive(Clone)]
pub struct AttestationProvider {
    generate: Arc<GenerateAttestationCallback>,
}

impl fmt::Debug for AttestationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("AttestationProvider").finish()
    }
}

impl AttestationProvider {
    pub fn new(generate: impl Fn() -> GenerateAttestationFuture + Send + Sync + 'static) -> Self {
        Self {
            generate: Arc::new(generate),
        }
    }

    pub(crate) async fn generate_header(&self) -> Option<HeaderValue> {
        HeaderValue::from_str(&(self.generate)().await?).ok()
    }
}
