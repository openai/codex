//! Sends registry requests with current credentials and at most one retry after a 401.
//! Transport failures and other HTTP errors are never replayed here.

use codex_http_client::HttpResponse;
use codex_http_client::Request;
use http::HeaderValue;
use http::StatusCode;

use super::EnvironmentRegistryClient;
use super::RegistryAuthMode;
use super::current_trace_context_headers;
use crate::ExecServerError;

impl EnvironmentRegistryClient {
    pub(super) async fn send_authenticated(
        &self,
        request: Request,
    ) -> Result<(HttpResponse, Option<HeaderValue>), ExecServerError> {
        let request = request
            .into_prepared()
            .map_err(ExecServerError::EnvironmentRegistryConfig)?;
        let mut retried = false;
        loop {
            let (authenticated, credential) =
                if self.auth_mode == RegistryAuthMode::ExecutorCredential {
                    let (headers, credential) = self.resolve_auth().await?;
                    let mut authenticated = request.clone();
                    authenticated.headers.extend(headers);
                    (authenticated, credential)
                } else {
                    (
                        self.auth_provider
                            .apply_auth(request.clone())
                            .await
                            .map_err(super::direct::direct_auth_error)?,
                        None,
                    )
                };
            let prepared = authenticated
                .prepare_body_for_send()
                .map_err(ExecServerError::EnvironmentRegistryConfig)?;
            let rejected_headers = prepared.headers.clone();
            let mut outgoing = self
                .http_for_auth()
                .request(authenticated.method, authenticated.url)
                .headers(prepared.headers)
                .headers(current_trace_context_headers())
                .body(prepared.body.unwrap_or_default());
            if let Some(timeout) = authenticated.timeout {
                outgoing = outgoing.timeout(timeout);
            }
            let response = outgoing.send().await?;
            if !retried
                && response.status() == StatusCode::UNAUTHORIZED
                && self
                    .auth_provider
                    .refresh_after_unauthorized(rejected_headers)
                    .await
            {
                retried = true;
                continue;
            }
            return Ok((response, credential));
        }
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
