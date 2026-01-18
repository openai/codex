//! SigV4-signing HTTP transport for AWS services.
//!
//! Wraps a base transport and signs all requests with AWS Signature Version 4.

use crate::aws_auth::AwsAuthProvider;
use crate::error::TransportError;
use crate::request::Request;
use crate::request::Response;
use crate::transport::HttpTransport;
use crate::transport::ReqwestTransport;
use crate::transport::StreamResponse;
use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sigv4::http_request::SignableBody;
use aws_sigv4::http_request::SignableRequest;
use aws_sigv4::http_request::SignatureLocation;
use aws_sigv4::http_request::SigningSettings;
use aws_sigv4::sign::v4;
use aws_smithy_runtime_api::client::identity::Identity;
use http::HeaderValue;
use std::time::SystemTime;
use tracing::debug;

/// HTTP transport that signs requests with AWS SigV4.
#[derive(Clone)]
pub struct SigV4Transport {
    inner: ReqwestTransport,
    auth: AwsAuthProvider,
    service: String,
    region: String,
}

impl std::fmt::Debug for SigV4Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigV4Transport")
            .field("service", &self.service)
            .field("region", &self.region)
            .finish_non_exhaustive()
    }
}

impl SigV4Transport {
    /// Create a new SigV4-signing transport.
    ///
    /// # Arguments
    /// * `client` - The underlying reqwest client
    /// * `auth` - AWS credentials provider
    /// * `service` - AWS service name (e.g., "bedrock")
    /// * `region` - AWS region (e.g., "us-east-1")
    pub fn new(
        client: reqwest::Client,
        auth: AwsAuthProvider,
        service: impl Into<String>,
        region: impl Into<String>,
    ) -> Self {
        Self {
            inner: ReqwestTransport::new(client),
            auth,
            service: service.into(),
            region: region.into(),
        }
    }

    /// Sign a request with AWS SigV4.
    async fn sign_request(&self, mut req: Request) -> Result<Request, TransportError> {
        let credentials = self
            .auth
            .credentials()
            .await
            .map_err(|e| TransportError::Network(format!("AWS credentials error: {e}")))?;

        // Convert AWS credentials to Identity for signing
        let aws_creds = Credentials::new(
            credentials.access_key_id(),
            credentials.secret_access_key(),
            credentials.session_token().map(|s| s.to_string()),
            credentials.expiry(),
            "codex-bedrock",
        );
        let identity: Identity = aws_creds.into();

        // Build signing settings
        let mut signing_settings = SigningSettings::default();
        signing_settings.signature_location = SignatureLocation::Headers;

        // Create signing params using the builder
        let signing_params = v4::signing_params::Builder::default()
            .identity(&identity)
            .region(&self.region)
            .name(&self.service)
            .time(SystemTime::now())
            .settings(signing_settings)
            .build()
            .map_err(|e| TransportError::Network(format!("SigV4 params error: {e}")))?
            .into();

        // Get the body bytes for signing
        let body_bytes = req
            .body
            .as_ref()
            .map(|b| serde_json::to_vec(b))
            .transpose()
            .map_err(|e| TransportError::Network(format!("Body serialization error: {e}")))?;

        let signable_body = match &body_bytes {
            Some(bytes) => SignableBody::Bytes(bytes.as_slice()),
            None => SignableBody::Bytes(&[]),
        };

        // Build a signable request
        let signable_request = SignableRequest::new(
            req.method.as_str(),
            &req.url,
            req.headers
                .iter()
                .map(|(k, v)| (k.as_str(), v.to_str().unwrap_or(""))),
            signable_body,
        )
        .map_err(|e| TransportError::Network(format!("Signable request error: {e}")))?;

        // Sign the request
        let (signing_instructions, _signature) =
            aws_sigv4::http_request::sign(signable_request, &signing_params)
                .map_err(|e| TransportError::Network(format!("SigV4 signing error: {e}")))?
                .into_parts();

        // Apply the signature headers to the request
        for (name, value) in signing_instructions.headers() {
            if let (Ok(header_name), Ok(header_value)) = (
                http::header::HeaderName::try_from(name),
                HeaderValue::from_str(value),
            ) {
                req.headers.insert(header_name, header_value);
            }
        }

        debug!(
            service = %self.service,
            region = %self.region,
            "Signed request with SigV4"
        );

        Ok(req)
    }
}

#[async_trait]
impl HttpTransport for SigV4Transport {
    async fn execute(&self, req: Request) -> Result<Response, TransportError> {
        let signed_req = self.sign_request(req).await?;
        self.inner.execute(signed_req).await
    }

    async fn stream(&self, req: Request) -> Result<StreamResponse, TransportError> {
        let signed_req = self.sign_request(req).await?;
        self.inner.stream(signed_req).await
    }
}
