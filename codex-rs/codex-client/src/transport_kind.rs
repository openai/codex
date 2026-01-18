//! Unified transport enum that can be either standard or SigV4-signing.

use crate::aws_auth::AwsAuthProvider;
use crate::error::TransportError;
use crate::request::Request;
use crate::request::Response;
use crate::sigv4_transport::SigV4Transport;
use crate::transport::HttpTransport;
use crate::transport::ReqwestTransport;
use crate::transport::StreamResponse;
use async_trait::async_trait;

/// Enum wrapper for different transport implementations.
///
/// This allows runtime selection of transport type based on provider configuration.
#[derive(Clone)]
pub enum TransportKind {
    /// Standard HTTP transport with bearer token auth.
    Standard(ReqwestTransport),
    /// AWS SigV4-signing transport for Bedrock.
    SigV4(SigV4Transport),
}

impl std::fmt::Debug for TransportKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard(_) => f.debug_tuple("Standard").finish(),
            Self::SigV4(t) => f.debug_tuple("SigV4").field(t).finish(),
        }
    }
}

impl TransportKind {
    /// Create a standard (non-SigV4) transport.
    pub fn standard(client: reqwest::Client) -> Self {
        Self::Standard(ReqwestTransport::new(client))
    }

    /// Create a SigV4-signing transport for AWS services.
    pub fn sigv4(
        client: reqwest::Client,
        auth: AwsAuthProvider,
        service: impl Into<String>,
        region: impl Into<String>,
    ) -> Self {
        Self::SigV4(SigV4Transport::new(client, auth, service, region))
    }
}

#[async_trait]
impl HttpTransport for TransportKind {
    async fn execute(&self, req: Request) -> Result<Response, TransportError> {
        match self {
            Self::Standard(t) => t.execute(req).await,
            Self::SigV4(t) => t.execute(req).await,
        }
    }

    async fn stream(&self, req: Request) -> Result<StreamResponse, TransportError> {
        match self {
            Self::Standard(t) => t.stream(req).await,
            Self::SigV4(t) => t.stream(req).await,
        }
    }
}
