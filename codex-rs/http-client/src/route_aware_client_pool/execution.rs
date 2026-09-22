//! Executes pooled HTTP requests, redirects, and TLS fallback within one request lifetime.

use std::future::Future;
use std::io;

use http::header::PROXY_AUTHORIZATION;

use super::RouteAwareClientPool;
use super::RouteAwareRequestError;
use super::SelectedTlsBackend;
use crate::OutboundProxyRoute;
use crate::request_draft::RequestDraft;
use crate::route_aware_redirect::MAX_REDIRECTS;
use crate::route_aware_redirect::insert_referer;
use crate::route_aware_redirect::is_redirect;
use crate::route_aware_redirect::redirect_request;
use crate::route_aware_redirect::redirect_url;
use crate::route_aware_redirect::remove_sensitive_headers;
use crate::tls_backend_fallback::should_retry_with_rustls;

impl RouteAwareClientPool {
    pub(crate) async fn send(
        &self,
        request: RequestDraft,
    ) -> Result<reqwest::Response, RouteAwareRequestError> {
        let http_client_factory = self.http_client_factory.clone();
        self.send_with_resolver(request, move |request_url| {
            let http_client_factory = http_client_factory.clone();
            async move {
                http_client_factory
                    .resolve_proxy_route_async(request_url)
                    .await
            }
        })
        .await
    }

    pub(super) async fn send_with_resolver<F, Fut>(
        &self,
        request: impl Into<RequestDraft>,
        resolve_route: F,
    ) -> Result<reqwest::Response, RouteAwareRequestError>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = io::Result<OutboundProxyRoute>>,
    {
        let draft = request.into();
        let mut request = draft.request;
        let mut draft_headers = Some(draft.headers);
        let request_method = request.method().clone();
        let mut request_url = request.url().to_string();
        let follows_redirects_manually = self.follows_redirects_manually();
        let timeout_deadline = request
            .timeout()
            .copied()
            .map(|timeout| tokio::time::Instant::now() + timeout);
        let mut redirects = 0;
        let mut previous_route = None;
        loop {
            let (current_route, client, selected_tls_backend) = match timeout_deadline {
                Some(timeout_deadline) => tokio::time::timeout_at(
                    timeout_deadline,
                    self.client_for_url_with_resolver(request.url().as_str(), &resolve_route),
                )
                .await
                .map_err(|_| RouteAwareRequestError::Timeout)??,
                None => {
                    self.client_for_url_with_resolver(request.url().as_str(), &resolve_route)
                        .await?
                }
            };
            if let Some(headers) = draft_headers.take() {
                request = RequestDraft { request, headers }.build(&client)?;
                request_url = request.url().to_string();
            }
            let current_url = request.url().clone();
            if previous_route
                .as_ref()
                .is_some_and(|previous_route| previous_route != &current_route)
            {
                request.headers_mut().remove(PROXY_AUTHORIZATION);
            }
            previous_route = Some(current_route.clone());
            if let Some(timeout_deadline) = timeout_deadline {
                let remaining = timeout_deadline
                    .checked_duration_since(tokio::time::Instant::now())
                    .ok_or(RouteAwareRequestError::Timeout)?;
                if remaining.is_zero() {
                    return Err(RouteAwareRequestError::Timeout);
                }
                *request.timeout_mut() = Some(remaining);
            }
            let method = request.method().clone();
            let headers = request.headers().clone();
            let version = request.version();
            let timeout = request.timeout().copied();
            let replay = request.try_clone();
            let execute_request = async {
                if follows_redirects_manually {
                    client.execute_without_request_logging(request).await
                } else {
                    client.execute(request).await
                }
            };
            let response = match match timeout_deadline {
                Some(timeout_deadline) => {
                    tokio::time::timeout_at(timeout_deadline, execute_request)
                        .await
                        .map_err(|_| RouteAwareRequestError::Timeout)?
                }
                None => execute_request.await,
            } {
                Ok(response) => response,
                Err(error) => {
                    let result = self
                        .retry_with_rustls(
                            &current_url,
                            &current_route,
                            selected_tls_backend,
                            replay.as_ref(),
                            error,
                            timeout_deadline,
                        )
                        .await;

                    if follows_redirects_manually
                        && let Err(RouteAwareRequestError::Request(error)) = &result
                    {
                        client.log_error_summary(&request_method, &request_url, error);
                    }

                    result?
                }
            };
            let status = response.status();
            if !follows_redirects_manually || !is_redirect(status) {
                if follows_redirects_manually {
                    client.log_response(&request_method, &request_url, &response);
                }
                return Ok(response);
            }
            let Some(next_url) = redirect_url(&response) else {
                if follows_redirects_manually {
                    client.log_response(&request_method, &request_url, &response);
                }
                return Ok(response);
            };
            let Some(mut next_request) =
                redirect_request(status, method, headers, version, timeout, replay, next_url)
            else {
                if follows_redirects_manually {
                    client.log_response(&request_method, &request_url, &response);
                }
                return Ok(response);
            };
            let next_request_url = next_request.url().clone();
            if !matches!(next_request_url.scheme(), "http" | "https") {
                return Err(RouteAwareRequestError::UnsupportedRedirectScheme(
                    next_request_url.scheme().to_string(),
                ));
            }
            if redirects >= MAX_REDIRECTS {
                return Err(RouteAwareRequestError::TooManyRedirects);
            }
            remove_sensitive_headers(next_request.headers_mut(), &current_url, &next_request_url);
            insert_referer(next_request.headers_mut(), &current_url, &next_request_url);
            request = next_request;
            redirects += 1;
        }
    }

    pub(super) async fn retry_with_rustls(
        &self,
        current_url: &reqwest::Url,
        current_route: &OutboundProxyRoute,
        selected_tls_backend: SelectedTlsBackend,
        replay: Option<&reqwest::Request>,
        error: reqwest::Error,
        timeout_deadline: Option<tokio::time::Instant>,
    ) -> Result<reqwest::Response, RouteAwareRequestError> {
        let Some(rustls_clients) = self.rustls_clients.as_ref() else {
            return Err(error.into());
        };

        if current_url.scheme() != "https"
            || selected_tls_backend == SelectedTlsBackend::RustlsFallback
            || !should_retry_with_rustls(&error)
        {
            return Err(error.into());
        }

        let Some(mut retry_request) = replay.and_then(reqwest::Request::try_clone) else {
            return Err(error.into());
        };

        let fallback_client = match rustls_clients.client_for_route(current_route) {
            Some(client) => client,
            None => self.rustls_client_for_route(current_route)?,
        };

        if let Some(timeout_deadline) = timeout_deadline {
            let remaining = timeout_deadline
                .checked_duration_since(tokio::time::Instant::now())
                .ok_or(RouteAwareRequestError::Timeout)?;
            if remaining.is_zero() {
                return Err(RouteAwareRequestError::Timeout);
            }
            *retry_request.timeout_mut() = Some(remaining);
        }

        let execute_retry = async {
            if self.follows_redirects_manually() {
                fallback_client
                    .execute_without_request_logging(retry_request)
                    .await
            } else {
                fallback_client.execute(retry_request).await
            }
        };
        let response = match timeout_deadline {
            Some(timeout_deadline) => tokio::time::timeout_at(timeout_deadline, execute_retry)
                .await
                .map_err(|_| RouteAwareRequestError::Timeout)?,
            None => execute_retry.await,
        }?;

        rustls_clients.remember(current_url, current_route, fallback_client);
        tracing::info!(
            event.name = "codex.http_client.tls_backend_fallback",
            "HTTP client switched to rustls after a TLS protocol negotiation failure"
        );

        Ok(response)
    }
}
