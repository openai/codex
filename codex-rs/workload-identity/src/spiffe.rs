//! Acquires JWT-SVIDs from a local SPIFFE Workload API without persisting credentials.

use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use spiffe::SpiffeId;
use spiffe::transport::Endpoint;
use tonic::codegen::http::uri::PathAndQuery;
use tonic::metadata::MetadataValue;
use url::Url;

use crate::WorkloadIdentityError;
use crate::assertion::validate_assertion;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Validated, process-owned settings for local SPIFFE JWT-SVID acquisition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpiffeWorkloadApi {
    endpoint: Endpoint,
    audience: String,
    spiffe_id: Option<SpiffeId>,
}

impl SpiffeWorkloadApi {
    pub fn new(
        endpoint: &str,
        audience: String,
        spiffe_id: Option<String>,
    ) -> Result<Self, WorkloadIdentityError> {
        if !cfg!(unix) {
            return Err(WorkloadIdentityError::UnsupportedSpiffePlatform);
        }
        let Endpoint::Unix(path) =
            Endpoint::parse(endpoint).map_err(|_| WorkloadIdentityError::InvalidSpiffeEndpoint)?
        else {
            return Err(WorkloadIdentityError::InvalidSpiffeEndpoint);
        };
        // The SPIFFE client leaves URI escapes in the path. Decode them before connecting.
        let path: PathBuf = Url::parse(&format!("file://{}", path.display()))
            .ok()
            .and_then(|url| url.to_file_path().ok())
            .filter(|path| !path.as_os_str().as_encoded_bytes().contains(&0))
            .ok_or(WorkloadIdentityError::InvalidSpiffeEndpoint)?;
        if audience.trim().is_empty() || audience.contains('\0') {
            return Err(WorkloadIdentityError::InvalidSpiffeAudience);
        }
        let spiffe_id = spiffe_id
            .map(SpiffeId::try_from)
            .transpose()
            .map_err(|_| WorkloadIdentityError::InvalidSpiffeId)?;
        Ok(Self {
            endpoint: Endpoint::Unix(path),
            audience,
            spiffe_id,
        })
    }

    pub(crate) async fn fetch_assertion(&self) -> Result<String, WorkloadIdentityError> {
        tokio::time::timeout(REQUEST_TIMEOUT, async {
            // Reconnect on each exchange so an agent restart cannot leave a stale channel.
            let channel = spiffe::transport::connect(&self.endpoint)
                .await
                .map_err(|_| WorkloadIdentityError::SpiffeUnavailable)?;
            let mut client = tonic::client::Grpc::new(channel);
            client
                .ready()
                .await
                .map_err(|_| WorkloadIdentityError::SpiffeUnavailable)?;
            let mut request = tonic::Request::new(JwtRequest {
                audience: vec![self.audience.clone()],
                spiffe_id: self
                    .spiffe_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            });
            request
                .metadata_mut()
                .insert("workload.spiffe.io", MetadataValue::from_static("true"));
            let response: tonic::Response<JwtResponse> = client
                .unary(
                    request,
                    PathAndQuery::from_static("/SpiffeWorkloadAPI/FetchJWTSVID"),
                    tonic_prost::ProstCodec::default(),
                )
                .await
                // Agent messages may contain credentials. Retain only the gRPC status code.
                .map_err(|status| WorkloadIdentityError::SpiffeRejected(status.code()))?;
            self.select_assertion(response.into_inner())
        })
        .await
        .map_err(|_| WorkloadIdentityError::SpiffeUnavailable)?
    }

    fn select_assertion(&self, response: JwtResponse) -> Result<String, WorkloadIdentityError> {
        let [svid] = response.svids.as_slice() else {
            return Err(if response.svids.len() > 1 {
                WorkloadIdentityError::AmbiguousSpiffeIdentity
            } else {
                WorkloadIdentityError::InvalidAssertion
            });
        };
        let assertion = validate_assertion(&svid.svid)?;
        let mut parts = assertion.split('.');
        let (Some(_), Some(payload), Some(_), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(WorkloadIdentityError::InvalidAssertion);
        };
        // Decode only selection claims. The token endpoint validates JOSE headers, signatures,
        // and expiry; in particular, JWT-SVIDs need not include a kid header.
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| WorkloadIdentityError::InvalidAssertion)?;
        let claims: JwtClaims = serde_json::from_slice(&payload)
            .map_err(|_| WorkloadIdentityError::InvalidAssertion)?;
        let spiffe_id =
            SpiffeId::try_from(claims.sub).map_err(|_| WorkloadIdentityError::InvalidAssertion)?;
        let audience_matches = match claims.aud {
            Audience::Single(audience) => audience == self.audience,
            Audience::Multiple(audiences) => audiences.contains(&self.audience),
        };
        if !audience_matches
            || self
                .spiffe_id
                .as_ref()
                .is_some_and(|expected| expected != &spiffe_id)
        {
            return Err(WorkloadIdentityError::InvalidAssertion);
        }
        Ok(assertion)
    }
}

#[derive(Deserialize)]
struct JwtClaims {
    sub: String,
    aud: Audience,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    Single(String),
    Multiple(Vec<String>),
}

// Only the FetchJWTSVID fields needed for acquisition are represented here.
#[derive(Clone, PartialEq, Eq, prost::Message)]
struct JwtRequest {
    #[prost(string, repeated, tag = "1")]
    audience: Vec<String>,
    #[prost(string, tag = "2")]
    spiffe_id: String,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
struct JwtResponse {
    #[prost(message, repeated, tag = "1")]
    svids: Vec<JwtSvid>,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
struct JwtSvid {
    #[prost(string, tag = "2")]
    svid: String,
}
