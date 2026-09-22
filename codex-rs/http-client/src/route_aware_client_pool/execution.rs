//! Executes pooled HTTP requests, redirects, and TLS fallback within one request lifetime.

use std::future::Future;
use std::io;
use std::sync::atomic::Ordering;

use futures::FutureExt;
use futures::future::BoxFuture;
use http::header::PROXY_AUTHORIZATION;

use super::RouteAwareClientPool;
use super::RouteAwareRequestError;
use super::SelectedTlsBackend;
use crate::HttpResponse;
use crate::OutboundProxyRoute;
use crate::client::apply_default_headers;
use crate::request_draft::RequestDraft;
use crate::route_aware_redirect::MAX_REDIRECTS;
use crate::route_aware_redirect::insert_referer;
use crate::route_aware_redirect::is_redirect;
use crate::route_aware_redirect::redirect_request;
use crate::route_aware_redirect::redirect_url;
use crate::route_aware_redirect::remove_sensitive_headers;
use crate::tls_backend_fallback::should_retry_with_rustls;

impl RouteAwareClientPool {
    /// Executes requests through shared routing, destination, and redirect checks.
    ///
    /// Box the execution future so nested callers do not inherit policy and routing state.
    pub fn execute(
        &self,
        request: reqwest::Request,
    ) -> BoxFuture<'_, Result<HttpResponse, RouteAwareRequestError>> {
        self.send(request.into())
    }

    pub(crate) fn send(
        &self,
        request: RequestDraft,
    ) -> BoxFuture<'_, Result<HttpResponse, RouteAwareRequestError>> {
        self.send_with_resolver(request, |request_url| {
            self.http_client_factory
                .resolve_proxy_route_async(request_url)
        })
        .boxed()
    }

    pub(super) async fn send_with_resolver<F, Fut>(
        &self,
        request: impl Into<RequestDraft>,
        resolve_route: F,
    ) -> Result<HttpResponse, RouteAwareRequestError>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = io::Result<OutboundProxyRoute>>,
    {
        let request = request.into();
        let permit = self
            .http_client_factory
            .network_policy()
            .acquire(request.request.url())?;
        permit
            .run(Box::pin(self.send_authorized(request, resolve_route)))
            .await?
    }

    async fn send_authorized<F, Fut>(
        &self,
        draft: RequestDraft,
        resolve_route: F,
    ) -> Result<HttpResponse, RouteAwareRequestError>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = io::Result<OutboundProxyRoute>>,
    {
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
            let current_url = request.url().clone();
            let permit = self
                .http_client_factory
                .network_policy()
                .acquire(&current_url)?;
            let hop = async {
                let (current_route, mut client, selected_tls_backend) = self
                    .client_for_url_with_resolver(current_url.as_str(), &resolve_route)
                    .await?;
                client.request_logging = self.client_builder.request_logging;
                if let Some(headers) = draft_headers.take() {
                    request = RequestDraft { request, headers }.build(&client)?;
                    request_url = request.url().to_string();
                    apply_default_headers(request.headers_mut(), &self.default_headers);
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
                let result = if follows_redirects_manually {
                    client.execute_without_request_logging(request).await
                } else {
                    client.execute(request).await
                };
                let response = match result {
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
                let next_request = if follows_redirects_manually && is_redirect(response.status()) {
                    redirect_url(&response).and_then(|next_url| {
                        redirect_request(
                            response.status(),
                            method,
                            headers,
                            version,
                            timeout,
                            replay,
                            next_url,
                        )
                    })
                } else {
                    None
                };
                if follows_redirects_manually && next_request.is_none() {
                    client.log_response(&request_method, &request_url, &response);
                }
                Ok::<_, RouteAwareRequestError>((response, next_request, current_url))
            };
            let (response, next_request, current_url) = permit
                .run(async {
                    match timeout_deadline {
                        Some(deadline) => tokio::time::timeout_at(deadline, hop)
                            .await
                            .map_err(|_| RouteAwareRequestError::Timeout)?,
                        None => hop.await,
                    }
                })
                .await??;
            let Some(mut next_request) = next_request else {
                return Ok(HttpResponse::new(response, permit));
            };
            if let Some(redirect_observed) = &self.client_builder.redirect_observed {
                redirect_observed.store(/*val*/ true, Ordering::Relaxed);
            }
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

        let mut fallback_client = match rustls_clients.client_for_route(current_route) {
            Some(client) => client,
            None => self.rustls_client_for_route(current_route)?,
        };
        fallback_client.request_logging = self.client_builder.request_logging;

        if let Some(timeout_deadline) = timeout_deadline {
            let remaining = timeout_deadline
                .checked_duration_since(tokio::time::Instant::now())
                .ok_or(RouteAwareRequestError::Timeout)?;
            if remaining.is_zero() {
                return Err(RouteAwareRequestError::Timeout);
            }
            *retry_request.timeout_mut() = Some(remaining);
        }

        let response = if self.follows_redirects_manually() {
            fallback_client
                .execute_without_request_logging(retry_request)
                .await
        } else {
            fallback_client.execute(retry_request).await
        }?;

        rustls_clients.remember(current_url, current_route, fallback_client);
        tracing::info!(
            event.name = "codex.http_client.tls_backend_fallback",
            "HTTP client switched to rustls after a TLS protocol negotiation failure"
        );

        Ok(response)
    }
}
