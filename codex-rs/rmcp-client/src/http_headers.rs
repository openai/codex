use std::collections::HashSet;
use std::fmt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
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
#[cfg(all(unix, not(target_os = "macos")))]
use codex_utils_pty::process_group::kill_process_group;
#[cfg(target_os = "macos")]
use codex_utils_pty::process_group::kill_process_group_with_member_fallback as kill_process_group;
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

const HELPER_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_HELPER_OUTPUT_BYTES: usize = 64 * 1024;
type CachedHeaders = Shared<BoxFuture<'static, std::result::Result<Arc<HeaderMap>, Arc<str>>>>;

struct HttpHeadersProvider {
    server_origin: Origin,
    cached: CachedHeaders,
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
        let cached = async move {
            run_helper(&command, &cwd)
                .await
                .map(Arc::new)
                .map_err(|error| Arc::<str>::from(error.to_string()))
        }
        .boxed()
        .shared();
        Ok(Self {
            server_origin: Url::parse(server_url)?.origin(),
            cached,
        })
    }

    async fn headers(&self) -> Result<Arc<HeaderMap>, ExecServerError> {
        self.cached
            .clone()
            .await
            .map_err(|error| ExecServerError::HttpRequest(error.to_string()))
    }
}

/// No rejection-driven refresh: 401/403 may be OAuth challenges, and reconnecting loses sessions.
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
        mut params: HttpRequestParams,
    ) -> Result<HttpRequestParams, ExecServerError> {
        let Ok(url) = Url::parse(&params.url) else {
            return Ok(params);
        };
        if self.provider.server_origin != url.origin() {
            return Ok(params);
        }
        // TODO: Follow same-origin redirects once later hops cannot leak helper headers.
        params.redirect_policy = HttpRedirectPolicy::Stop;

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
        for (name, value) in headers.iter() {
            params
                .headers
                .retain(|header| !header.name.eq_ignore_ascii_case(name.as_str()));
            params.headers.push(HttpHeader {
                name: name.to_string(),
                value: std::str::from_utf8(value.as_bytes())
                    .map_err(|error| ExecServerError::HttpRequest(error.to_string()))?
                    .to_string(),
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
}

impl HttpClient for HttpHeadersClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        async move {
            let params = self.prepare_request(params).await?;
            self.inner.http_request(params).await
        }
        .boxed()
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        async move {
            let params = self.prepare_request(params).await?;
            self.inner.http_request_stream(params).await
        }
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
        // Helper values replace same-name configured headers; bearer/OAuth owns Authorization.
        // Google IAP uses Proxy-Authorization alongside application Authorization. For HTTPS MCP
        // URLs it is sent through the forward-proxy tunnel to IAP, not used as CONNECT auth.
        if matches!(
            name.as_str(),
            "accept"
                | "authorization"
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
