use std::future::Future;
use std::pin::Pin;

pub(crate) const X_OAI_ATTESTATION_HEADER: &str = "x-oai-attestation";

pub type GenerateAttestationFuture<'a> = Pin<Box<dyn Future<Output = Option<String>> + Send + 'a>>;

/// Request context that host integrations can use when deciding whether to
/// generate an attestation header value.
#[derive(Clone, Copy, Debug)]
pub struct AttestationContext {
    pub uses_chatgpt_auth: bool,
}

/// Host integration boundary for just-in-time attestation header values.
///
/// Implementations own the policy for when attestation should be attempted and
/// return the opaque string expected by the upstream `x-oai-attestation`
/// header. Core only forwards valid HTTP header values returned by the host.
pub trait AttestationProvider: std::fmt::Debug + Send + Sync {
    fn generate_header_value(&self, context: AttestationContext) -> GenerateAttestationFuture<'_>;
}
