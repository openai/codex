use std::collections::HashSet;
use std::fmt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
#[cfg(unix)]
use codex_utils_pty::process_group::kill_process_group;
use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::Shared;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use serde::Deserialize;
use serde::de::MapAccess;
use serde::de::Visitor;
use tokio::io::AsyncReadExt;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::Instant;
use url::Origin;
use url::Url;

use crate::utils::create_env_for_mcp_server;
use crate::www_authenticate::insufficient_scope_challenge;

const HELPER_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_HELPER_OUTPUT_BYTES: usize = 64 * 1024;
type CachedHeaders = Shared<BoxFuture<'static, std::result::Result<Arc<HeaderMap>, Arc<str>>>>;

struct HttpHeadersProvider {
    server_origin: Origin,
    command: String,
    cwd: PathBuf,
    cache: Mutex<HeadersCache>,
}

struct HeadersCache {
    // Identifies a cohort of rejected requests, not a credential version. Advance even after
    // failed or unchanged refreshes so concurrent rejections share one helper invocation.
    refresh_epoch: u64,
    current: CachedHeaders,
    refresh: Option<CachedHeaders>,
}

struct RequestHeaders {
    refresh_epoch: u64,
    values: Arc<HeaderMap>,
}

struct HttpHeadersClient {
    inner: Arc<dyn HttpClient>,
    provider: HttpHeadersProvider,
}

struct HelperProcess {
    child: Child,
    #[cfg(unix)]
    process_group_id: u32,
    #[cfg(windows)]
    job: codex_utils_pty::JobObject,
}

struct RawHeaderEntries {
    // Keep raw entries because ordinary map deserialization collapses exact duplicate keys.
    entries: Vec<(String, String)>,
    has_exact_duplicate: bool,
}

struct RawHeaderEntriesVisitor;

impl<'de> Visitor<'de> for RawHeaderEntriesVisitor {
    type Value = RawHeaderEntries;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object of string header names and values")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entries = Vec::with_capacity(map.size_hint().unwrap_or_default());
        let mut names = HashSet::new();
        let mut has_exact_duplicate = false;
        while let Some((name, value)) = map.next_entry::<String, String>()? {
            has_exact_duplicate |= !names.insert(name.clone());
            entries.push((name, value));
        }
        Ok(RawHeaderEntries {
            entries,
            has_exact_duplicate,
        })
    }
}

impl<'de> Deserialize<'de> for RawHeaderEntries {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(RawHeaderEntriesVisitor)
    }
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = kill_process_group(self.process_group_id);

        #[cfg(windows)]
        let _ = self.job.terminate();

        let _ = self.child.start_kill();
    }
}

impl HttpHeadersProvider {
    fn new(server_url: &str, command: &str, cwd: PathBuf) -> Result<Self> {
        let command = command.to_string();
        let cached = Self::helper_attempt(command.clone(), cwd.clone());
        Ok(Self {
            server_origin: Url::parse(server_url)?.origin(),
            command,
            cwd,
            cache: Mutex::new(HeadersCache {
                refresh_epoch: 0,
                current: cached,
                refresh: None,
            }),
        })
    }

    fn helper_attempt(command: String, cwd: PathBuf) -> CachedHeaders {
        async move {
            // Keep helper cleanup running after caller cancellation; dropping the set aborts it.
            let mut tasks = tokio::task::JoinSet::new();
            tasks.spawn(async move {
                run_helper(&command, &cwd)
                    .await
                    .map(Arc::new)
                    .map_err(|error| Arc::<str>::from(error.to_string()))
            });
            tasks
                .join_next()
                .await
                .ok_or_else(|| Arc::<str>::from("MCP HTTP headers helper task was unavailable"))?
                .map_err(|_| Arc::<str>::from("MCP HTTP headers helper task failed"))?
        }
        .boxed()
        .shared()
    }

    async fn headers(&self) -> Result<RequestHeaders, ExecServerError> {
        let (refresh_epoch, current) = {
            let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
            (cache.refresh_epoch, cache.current.clone())
        };
        current
            .await
            .map(|values| RequestHeaders {
                refresh_epoch,
                values,
            })
            .map_err(|error| ExecServerError::HttpRequest(error.to_string()))
    }

    async fn refresh(&self, rejected_epoch: u64) -> Result<Arc<HeaderMap>, ExecServerError> {
        let attempt = {
            let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
            if cache.refresh_epoch != rejected_epoch {
                cache.current.clone()
            } else {
                cache
                    .refresh
                    .get_or_insert_with(|| {
                        Self::helper_attempt(self.command.clone(), self.cwd.clone())
                    })
                    .clone()
            }
        };
        let result = attempt.clone().await;
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.refresh_epoch == rejected_epoch
            && cache
                .refresh
                .as_ref()
                .is_some_and(|refresh| refresh.ptr_eq(&attempt))
        {
            cache.refresh_epoch = rejected_epoch.saturating_add(/*rhs*/ 1);
            if result.is_ok() {
                cache.current = attempt;
            }
            cache.refresh = None;
        }
        result.map_err(|error| ExecServerError::HttpRequest(error.to_string()))
    }
}

/// Refreshes helper headers once after a same-origin POST returns 401/403, retrying only
/// when helper-provided values change. OAuth challenges survive failed or unchanged refreshes.
/// Helper Authorization is used only when the request has no explicit bearer/OAuth credential.
/// The helper is a short-lived process; independently managed daemons remain caller-owned.
pub fn with_http_headers_helper(
    inner: Arc<dyn HttpClient>,
    server_url: &str,
    command: &str,
    cwd: PathBuf,
) -> Result<Arc<dyn HttpClient>> {
    let provider = HttpHeadersProvider::new(server_url, command, cwd)?;
    Ok(Arc::new(HttpHeadersClient { inner, provider }))
}

impl HttpHeadersClient {
    async fn prepare_request(
        &self,
        params: HttpRequestParams,
    ) -> Result<(HttpRequestParams, Option<RequestHeaders>, Option<Instant>), ExecServerError> {
        let Ok(url) = Url::parse(&params.url) else {
            return Ok((params, None, None));
        };
        if self.provider.server_origin != url.origin() {
            return Ok((params, None, None));
        }

        let deadline = params
            .timeout_ms
            .map(|timeout_ms| Instant::now() + Duration::from_millis(timeout_ms));
        let headers = match deadline {
            Some(deadline) => tokio::time::timeout_at(deadline, self.provider.headers())
                .await
                .map_err(|_| {
                    ExecServerError::HttpRequest("HTTP request timed out".to_string())
                })??,
            None => self.provider.headers().await?,
        };
        let params = self.apply_headers(params, &headers.values, deadline)?;
        Ok((params, Some(headers), deadline))
    }

    fn apply_headers(
        &self,
        mut params: HttpRequestParams,
        headers: &HeaderMap,
        deadline: Option<Instant>,
    ) -> Result<HttpRequestParams, ExecServerError> {
        // TODO: Follow same-origin redirects once later hops cannot leak helper headers.
        params.redirect_policy = HttpRedirectPolicy::Stop;
        for (name, value) in headers {
            // Explicit bearer tokens, MCP OAuth and token-endpoint client authentication win.
            if name == http::header::AUTHORIZATION
                && params
                    .headers
                    .iter()
                    .any(|header| header.name.eq_ignore_ascii_case("authorization"))
            {
                continue;
            }
            params
                .headers
                .retain(|header| !header.name.eq_ignore_ascii_case(name.as_str()));
            params.headers.push(HttpHeader {
                name: name.to_string(),
                value: std::str::from_utf8(value.as_bytes())
                    .map_err(|error| ExecServerError::HttpRequest(error.to_string()))?
                    .to_string(),
                value_env_var: None,
            });
        }
        if let Some(deadline) = deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            params.timeout_ms = Some(
                u64::try_from(remaining.as_millis())
                    .unwrap_or(u64::MAX)
                    .max(1),
            );
        }
        Ok(params)
    }

    fn reject_proxy_authorization_redirect(
        response: &HttpRequestResponse,
    ) -> Result<(), ExecServerError> {
        if (300..400).contains(&response.status)
            && response
                .headers
                .iter()
                .any(|header| header.name.eq_ignore_ascii_case("location"))
        {
            return Err(ExecServerError::HttpRequest(
                "MCP HTTP redirect cannot safely replay Proxy-Authorization credentials"
                    .to_string(),
            ));
        }
        Ok(())
    }

    async fn retry_request(
        &self,
        params: HttpRequestParams,
        rejected: RequestHeaders,
        deadline: Option<Instant>,
        response: &HttpRequestResponse,
    ) -> Option<HttpRequestParams> {
        if !matches!(response.status, 401 | 403)
            || response.status == 403 && insufficient_scope_challenge(&response.headers).is_some()
        {
            return None;
        }
        // A rejection may be an OAuth challenge, so preserve it if the helper cannot improve it.
        let refreshed = match deadline {
            Some(deadline) => {
                tokio::time::timeout_at(deadline, self.provider.refresh(rejected.refresh_epoch))
                    .await
                    .ok()?
                    .ok()?
            }
            None => self.provider.refresh(rejected.refresh_epoch).await.ok()?,
        };
        // Header names are user-defined. Compare effective helper values, excluding an
        // Authorization that would be overridden, without treating JSON key order as a change.
        let explicit_authorization = params
            .headers
            .iter()
            .any(|header| header.name.eq_ignore_ascii_case("authorization"));
        let differs = |left: &HeaderMap, right: &HeaderMap| {
            left.iter().any(|(name, value)| {
                (!explicit_authorization || name != http::header::AUTHORIZATION)
                    && right.get(name) != Some(value)
            })
        };
        if !differs(&rejected.values, &refreshed) && !differs(&refreshed, &rejected.values) {
            return None;
        }
        self.apply_headers(params, &refreshed, deadline).ok()
    }

    async fn request<'a, T>(
        &'a self,
        params: HttpRequestParams,
        send: impl Fn(HttpRequestParams) -> BoxFuture<'a, Result<T, ExecServerError>>,
        response_headers: impl Fn(&T) -> &HttpRequestResponse,
    ) -> Result<T, ExecServerError> {
        // OAuth validates redirect responses itself; only intercept MCP replay.
        let mcp_redirect_was_stopped = params.redirect_policy == HttpRedirectPolicy::Stop
            && !params.request_id.starts_with("oauth-request-");
        let original_headers = params
            .method
            .eq_ignore_ascii_case("POST")
            .then(|| params.headers.clone());
        let (params, headers, deadline) = self.prepare_request(params).await?;
        let original = original_headers
            .filter(|_| headers.is_some())
            .map(|headers| {
                let mut original = params.clone();
                original.headers = headers;
                original
            });
        let needs_redirect_check = |params: &HttpRequestParams| {
            mcp_redirect_was_stopped
                && Url::parse(&params.url).is_ok_and(|url| url.scheme() == "http")
                && params
                    .headers
                    .iter()
                    .any(|header| header.name.eq_ignore_ascii_case("proxy-authorization"))
        };
        let prevent_proxy_authorization_redirect = needs_redirect_check(&params);
        let response = send(params).await?;
        if prevent_proxy_authorization_redirect {
            Self::reject_proxy_authorization_redirect(response_headers(&response))?;
        }
        if let (Some(original), Some(headers)) = (original, headers)
            && let Some(retry) = self
                .retry_request(original, headers, deadline, response_headers(&response))
                .await
        {
            drop(response);
            let prevent_proxy_authorization_redirect = needs_redirect_check(&retry);
            let response = send(retry).await?;
            if prevent_proxy_authorization_redirect {
                Self::reject_proxy_authorization_redirect(response_headers(&response))?;
            }
            return Ok(response);
        }
        Ok(response)
    }
}

impl HttpClient for HttpHeadersClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        self.request(
            params,
            |params| self.inner.http_request(params),
            |response| response,
        )
        .boxed()
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        self.request(
            params,
            |params| self.inner.http_request_stream(params),
            |response| &response.0,
        )
        .boxed()
    }
}

async fn run_helper(command: &str, cwd: &Path) -> Result<HeaderMap> {
    #[cfg(windows)]
    let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
    #[cfg(not(windows))]
    let shell = "sh";

    // Match the repository's existing shell-command convention. The command is ordinary
    // configuration and may be visible in local process metadata; credentials belong in the
    // JSON output rather than in the command text.
    let mut process = Command::new(shell);
    #[cfg(windows)]
    {
        process.args(["/Q", "/D", "/C"]);
        process.as_std_mut().raw_arg(format!(r#""{command}""#));
    }
    #[cfg(not(windows))]
    process.args(["-c", command]);
    #[cfg(unix)]
    process.process_group(0);
    process
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(cwd)
        // Match local MCP subprocess policy; arbitrary ambient variables are not inherited.
        .env_clear()
        .envs(create_env_for_mcp_server(/*extra_env*/ None, &[])?)
        .kill_on_drop(true);

    #[cfg(windows)]
    let (child, job) = {
        let job = codex_utils_pty::JobObject::create_without_breakaway()
            .map_err(|error| anyhow!("MCP HTTP headers helper containment failed: {error}"))?;
        let child = job
            .spawn_contained(&mut process)
            .map_err(|error| anyhow!("MCP HTTP headers helper failed to start: {error}"))?;
        (child, job)
    };
    #[cfg(not(windows))]
    let child = process
        .spawn()
        .map_err(|error| anyhow!("MCP HTTP headers helper failed to start: {error}"))?;
    let mut process = HelperProcess {
        #[cfg(unix)]
        process_group_id: child
            .id()
            .ok_or_else(|| anyhow!("MCP HTTP headers helper process id was unavailable"))?,
        child,
        #[cfg(windows)]
        job,
    };
    let output = tokio::time::timeout(HELPER_TIMEOUT, async {
        let stdout = process
            .child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("MCP HTTP headers helper stdout was unavailable"))?;
        let mut output = Vec::new();
        stdout
            .take((MAX_HELPER_OUTPUT_BYTES + 1) as u64)
            .read_to_end(&mut output)
            .await?;
        if output.len() > MAX_HELPER_OUTPUT_BYTES {
            return Err(anyhow!("MCP HTTP headers helper output exceeds 64 KiB"));
        }
        let status = process.child.wait().await?;
        if !status.success() {
            return Err(anyhow!(
                "MCP HTTP headers helper exited with status {status}"
            ));
        }
        Ok(output)
    })
    .await
    .map_err(|_| anyhow!("MCP HTTP headers helper timed out after 10 seconds"))??;

    parse_helper_output(output)
}

fn parse_helper_output(stdout: Vec<u8>) -> Result<HeaderMap> {
    let stdout = String::from_utf8(stdout)
        .map_err(|_| anyhow!("MCP HTTP headers helper wrote non-UTF-8 data"))?;
    let mut deserializer = serde_json::Deserializer::from_str(stdout.trim());
    let headers = RawHeaderEntries::deserialize(&mut deserializer)
        .and_then(|headers| {
            deserializer.end()?;
            Ok(headers)
        })
        .map_err(|_| anyhow!("MCP HTTP headers helper must output a JSON object of strings"))?;
    if headers.has_exact_duplicate {
        return Err(anyhow!(
            "MCP HTTP headers helper returned duplicate header names"
        ));
    }
    let mut parsed = HeaderMap::with_capacity(headers.entries.len());
    for (name, value) in headers.entries {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| anyhow!("MCP HTTP headers helper returned an invalid header name"))?;
        // Helper values replace same-name configured headers, except explicit Authorization.
        // Google IAP uses Proxy-Authorization alongside application Authorization. For HTTPS MCP
        // URLs it is sent through the forward-proxy tunnel to IAP, not used as CONNECT auth.
        if matches!(
            name.as_str(),
            "accept"
                | "connection"
                | "content-encoding"
                | "content-length"
                | "content-type"
                | "host"
                | "keep-alive"
                | "last-event-id"
                | "mcp-protocol-version"
                | "mcp-session-id"
                | "origin"
                | "proxy-connection"
                | "referer"
                | "te"
                | "trailer"
                | "transfer-encoding"
                | "upgrade"
        ) {
            return Err(anyhow!(
                "MCP HTTP headers helper returned a reserved header"
            ));
        }
        if parsed.contains_key(&name) {
            return Err(anyhow!(
                "MCP HTTP headers helper returned duplicate header names"
            ));
        }
        let value = HeaderValue::from_str(&value)
            .map_err(|_| anyhow!("MCP HTTP headers helper returned an invalid header value"))?;
        parsed.insert(name, value);
    }
    Ok(parsed)
}

#[cfg(test)]
#[path = "http_headers_tests.rs"]
mod tests;
