use crate::program_resolver;

use anyhow::Result;
use anyhow::anyhow;
use codex_config::McpServerHttpHeadersHelperConfig;
use codex_config::types::McpServerEnvVar;
use reqwest::ClientBuilder;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

const MAX_HTTP_HEADERS_HELPER_STDOUT_BYTES: usize = 64 * 1024;
const MAX_HTTP_HEADERS_HELPER_STDERR_BYTES: usize = 8 * 1024;
const MAX_HTTP_HEADERS_HELPER_HEADERS: usize = 64;
const MAX_HTTP_HEADERS_HELPER_HEADER_VALUE_BYTES: usize = 16 * 1024;

pub(crate) fn create_env_for_mcp_server(
    extra_env: Option<HashMap<OsString, OsString>>,
    env_vars: &[McpServerEnvVar],
) -> Result<HashMap<OsString, OsString>> {
    let additional_env_vars = local_stdio_env_var_names(env_vars)?;
    let env = DEFAULT_ENV_VARS
        .iter()
        .copied()
        .chain(additional_env_vars)
        .filter_map(|var| env::var_os(var).map(|value| (OsString::from(var), value)))
        .chain(extra_env.unwrap_or_default())
        .collect();
    Ok(env)
}

pub(crate) fn create_env_overlay_for_remote_mcp_server(
    extra_env: Option<HashMap<OsString, OsString>>,
    env_vars: &[McpServerEnvVar],
) -> HashMap<OsString, OsString> {
    // Remote stdio should inherit PATH/HOME/etc. from the executor side, not
    // from the orchestrator process. Only forward variables explicitly named
    // by the MCP config plus literal env overrides from that config.
    env_vars
        .iter()
        .filter(|var| !var.is_remote_source())
        .filter_map(|var| env::var_os(var.name()).map(|value| (OsString::from(var.name()), value)))
        .chain(extra_env.unwrap_or_default())
        .collect()
}

pub(crate) fn remote_mcp_env_var_names(env_vars: &[McpServerEnvVar]) -> Vec<String> {
    env_vars
        .iter()
        .filter(|var| var.is_remote_source())
        .map(|var| var.name().to_string())
        .collect()
}

fn local_stdio_env_var_names(env_vars: &[McpServerEnvVar]) -> Result<impl Iterator<Item = &str>> {
    if let Some(remote_var) = env_vars.iter().find(|var| var.is_remote_source()) {
        return Err(anyhow!(
            "env_vars entry `{}` uses source `remote`, which requires remote MCP stdio",
            remote_var.name()
        ));
    }
    Ok(env_vars.iter().map(McpServerEnvVar::name))
}

pub(crate) async fn build_default_headers(
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    http_headers_helper: Option<McpServerHttpHeadersHelperConfig>,
) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();

    if let Some(static_headers) = http_headers {
        for (name, value) in static_headers {
            let header_name = match HeaderName::from_bytes(name.as_bytes()) {
                Ok(name) => name,
                Err(err) => {
                    tracing::warn!("invalid HTTP header name `{name}`: {err}");
                    continue;
                }
            };
            let header_value = match HeaderValue::from_str(value.as_str()) {
                Ok(value) => value,
                Err(err) => {
                    tracing::warn!("invalid HTTP header value for `{name}`: {err}");
                    continue;
                }
            };
            headers.insert(header_name, header_value);
        }
    }

    if let Some(env_headers) = env_http_headers {
        for (name, env_var) in env_headers {
            if let Ok(value) = env::var(&env_var) {
                if value.trim().is_empty() {
                    continue;
                }

                let header_name = match HeaderName::from_bytes(name.as_bytes()) {
                    Ok(name) => name,
                    Err(err) => {
                        tracing::warn!("invalid HTTP header name `{name}`: {err}");
                        continue;
                    }
                };

                let header_value = match HeaderValue::from_str(value.as_str()) {
                    Ok(value) => value,
                    Err(err) => {
                        tracing::warn!(
                            "invalid HTTP header value read from {env_var} for `{name}`: {err}"
                        );
                        continue;
                    }
                };
                headers.insert(header_name, header_value);
            }
        }
    }

    if let Some(helper) = http_headers_helper {
        let helper_headers = run_http_headers_helper(&helper).await?;
        if helper_headers.len() > MAX_HTTP_HEADERS_HELPER_HEADERS {
            return Err(anyhow!(
                "MCP HTTP headers helper `{}` produced {} headers, exceeding the limit of {}",
                helper.command,
                helper_headers.len(),
                MAX_HTTP_HEADERS_HELPER_HEADERS
            ));
        }
        for (name, value) in helper_headers {
            let header_name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|err| anyhow!("invalid HTTP header name `{name}` from helper: {err}"))?;
            if value.len() > MAX_HTTP_HEADERS_HELPER_HEADER_VALUE_BYTES {
                return Err(anyhow!(
                    "HTTP header value from helper for `{name}` exceeds the limit of {MAX_HTTP_HEADERS_HELPER_HEADER_VALUE_BYTES} bytes"
                ));
            }
            let header_value = HeaderValue::from_str(value.as_str()).map_err(|err| {
                anyhow!("invalid HTTP header value from helper for `{name}`: {err}")
            })?;
            headers.insert(header_name, header_value);
        }
    }

    Ok(headers)
}

async fn run_http_headers_helper(
    helper: &McpServerHttpHeadersHelperConfig,
) -> Result<HashMap<String, String>> {
    let program = resolve_http_headers_helper_program(helper)?;
    let mut command = Command::new(program);
    command
        .args(&helper.args)
        .current_dir(helper.cwd.as_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command.spawn().map_err(|err| {
        anyhow!(
            "MCP HTTP headers helper `{}` failed to start: {err}",
            helper.command
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        anyhow!(
            "MCP HTTP headers helper `{}` failed to capture stdout",
            helper.command
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        anyhow!(
            "MCP HTTP headers helper `{}` failed to capture stderr",
            helper.command
        )
    })?;
    let stdout_task = tokio::spawn(read_capped(stdout, MAX_HTTP_HEADERS_HELPER_STDOUT_BYTES));
    let stderr_task = tokio::spawn(read_capped(stderr, MAX_HTTP_HEADERS_HELPER_STDERR_BYTES));

    let status = match tokio::time::timeout(helper.timeout(), child.wait()).await {
        Ok(result) => result.map_err(|err| {
            anyhow!(
                "MCP HTTP headers helper `{}` failed while waiting for exit: {err}",
                helper.command
            )
        })?,
        Err(_) => {
            let _ = child.kill().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(anyhow!(
                "MCP HTTP headers helper `{}` timed out after {} ms",
                helper.command,
                helper.timeout_ms.get()
            ));
        }
    };

    let stdout =
        join_capped_output(&helper.command, stdout_task, "stdout", helper.timeout()).await?;
    let stderr =
        join_capped_output(&helper.command, stderr_task, "stderr", helper.timeout()).await?;

    if !status.success() {
        let stderr_suffix = if stderr.bytes.is_empty() {
            String::new()
        } else {
            format!(
                "; stderr omitted ({} byte{})",
                stderr.bytes.len(),
                if stderr.bytes.len() == 1 { "" } else { "s" }
            )
        };
        return Err(anyhow!(
            "MCP HTTP headers helper `{}` exited with status {status}{stderr_suffix}",
            helper.command
        ));
    }

    if stdout.truncated {
        return Err(anyhow!(
            "MCP HTTP headers helper `{}` wrote more than {} bytes to stdout",
            helper.command,
            MAX_HTTP_HEADERS_HELPER_STDOUT_BYTES
        ));
    }

    let stdout = String::from_utf8(stdout.bytes).map_err(|_| {
        anyhow!(
            "MCP HTTP headers helper `{}` wrote non-UTF-8 data to stdout",
            helper.command
        )
    })?;
    let output = stdout.trim();
    if output.is_empty() {
        return Err(anyhow!(
            "MCP HTTP headers helper `{}` produced empty output",
            helper.command
        ));
    }

    serde_json::from_str(output).map_err(|err| {
        anyhow!(
            "MCP HTTP headers helper `{}` must output a JSON object with string values: {err}",
            helper.command
        )
    })
}

fn resolve_http_headers_helper_program(
    helper: &McpServerHttpHeadersHelperConfig,
) -> Result<OsString> {
    let command = Path::new(&helper.command);
    if command.is_absolute() {
        return Ok(command.as_os_str().to_os_string());
    }
    if has_path_separator(command.as_os_str()) {
        return Ok(helper.cwd.as_path().join(command).into_os_string());
    }

    program_resolver::resolve(
        command.as_os_str().to_os_string(),
        &env::vars_os().collect(),
        helper.cwd.as_path(),
    )
    .map_err(|err| {
        anyhow!(
            "MCP HTTP headers helper `{}` could not be resolved: {err}",
            helper.command
        )
    })
}

fn has_path_separator(value: &OsStr) -> bool {
    let path = PathBuf::from(value);
    path.components().count() > 1
}

struct CappedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

async fn read_capped<R>(mut reader: R, max_bytes: usize) -> io::Result<CappedOutput>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut chunk = [0_u8; 8192];

    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            break;
        }

        let remaining = max_bytes.saturating_sub(bytes.len());
        if remaining == 0 {
            truncated = true;
            continue;
        }

        let to_copy = remaining.min(read);
        bytes.extend_from_slice(&chunk[..to_copy]);
        if to_copy < read {
            truncated = true;
        }
    }

    Ok(CappedOutput { bytes, truncated })
}

async fn join_capped_output(
    command: &str,
    mut task: tokio::task::JoinHandle<io::Result<CappedOutput>>,
    stream_name: &str,
    timeout: Duration,
) -> Result<CappedOutput> {
    tokio::select! {
        result = &mut task => {
            result
                .map_err(|err| anyhow!("MCP HTTP headers helper `{command}` {stream_name} task failed: {err}"))?
                .map_err(|err| anyhow!("MCP HTTP headers helper `{command}` failed to read {stream_name}: {err}"))
        }
        _ = tokio::time::sleep(timeout) => {
            task.abort();
            Err(anyhow!("MCP HTTP headers helper `{command}` timed out while reading {stream_name}"))
        }
    }
}

pub(crate) fn apply_default_headers(
    builder: ClientBuilder,
    default_headers: &HeaderMap,
) -> ClientBuilder {
    if default_headers.is_empty() {
        builder
    } else {
        builder.default_headers(default_headers.clone())
    }
}

#[cfg(unix)]
pub(crate) const DEFAULT_ENV_VARS: &[&str] = &[
    "HOME",
    "LOGNAME",
    "PATH",
    "SHELL",
    "USER",
    "__CF_USER_TEXT_ENCODING",
    "LANG",
    "LC_ALL",
    "TERM",
    "TMPDIR",
    "TZ",
];

#[cfg(windows)]
pub(crate) const DEFAULT_ENV_VARS: &[&str] =
    codex_protocol::shell_environment::WINDOWS_CORE_ENV_VARS;

#[cfg(test)]
mod tests {
    use super::*;
    use codex_config::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    use serial_test::serial;
    use std::ffi::OsStr;
    use std::path::Path;
    use std::path::PathBuf;

    struct EnvVarGuard {
        key: String,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &str, value: impl AsRef<OsStr>) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value.as_ref());
            }
            Self {
                key: key.to_string(),
                original,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                unsafe {
                    std::env::set_var(&self.key, value);
                }
            } else {
                unsafe {
                    std::env::remove_var(&self.key);
                }
            }
        }
    }

    #[tokio::test]
    async fn create_env_honors_overrides() {
        let value = "custom".to_string();
        let expected = OsString::from(&value);
        let env = create_env_for_mcp_server(
            Some(HashMap::from([(OsString::from("TZ"), expected.clone())])),
            &[],
        )
        .expect("local MCP env should build");
        assert_eq!(env.get(OsStr::new("TZ")), Some(&expected));
    }

    #[test]
    #[serial(extra_rmcp_env)]
    fn create_env_includes_additional_whitelisted_variables() {
        let custom_var = "EXTRA_RMCP_ENV";
        let value = "from-env";
        let expected = OsString::from(value);
        let _guard = EnvVarGuard::set(custom_var, value);
        let env = create_env_for_mcp_server(/*extra_env*/ None, &[custom_var.into()])
            .expect("local MCP env should build");
        assert_eq!(env.get(OsStr::new(custom_var)), Some(&expected));
    }

    #[test]
    #[serial(extra_rmcp_env)]
    fn create_remote_env_overlay_only_forwards_explicit_variables() {
        let default_var = DEFAULT_ENV_VARS[0];
        let custom_var = "EXTRA_REMOTE_RMCP_ENV";
        let custom_value = OsString::from("from-env");
        let _default_guard = EnvVarGuard::set(default_var, "from-default");
        let _custom_guard = EnvVarGuard::set(custom_var, &custom_value);

        let env =
            create_env_overlay_for_remote_mcp_server(/*extra_env*/ None, &[custom_var.into()]);

        assert_eq!(
            env,
            HashMap::from([(OsString::from(custom_var), custom_value)])
        );
    }

    #[test]
    #[serial(extra_rmcp_env)]
    fn create_remote_env_overlay_does_not_copy_remote_source_variables() {
        let remote_var = "REMOTE_ONLY_RMCP_ENV";
        let local_var = "LOCAL_RMCP_ENV";
        let local_value = OsString::from("from-local-env");
        let _remote_guard = EnvVarGuard::set(remote_var, "should-not-be-copied");
        let _local_guard = EnvVarGuard::set(local_var, &local_value);

        let env = create_env_overlay_for_remote_mcp_server(
            /*extra_env*/ None,
            &[
                McpServerEnvVar::Config {
                    name: remote_var.to_string(),
                    source: Some("remote".to_string()),
                },
                McpServerEnvVar::Config {
                    name: local_var.to_string(),
                    source: Some("local".to_string()),
                },
            ],
        );

        assert_eq!(
            env,
            HashMap::from([(OsString::from(local_var), local_value)])
        );
    }

    #[test]
    fn remote_mcp_env_var_names_returns_remote_source_names() {
        let names = remote_mcp_env_var_names(&[
            "LEGACY".into(),
            McpServerEnvVar::Config {
                name: "LOCAL".to_string(),
                source: Some("local".to_string()),
            },
            McpServerEnvVar::Config {
                name: "REMOTE".to_string(),
                source: Some("remote".to_string()),
            },
        ]);

        assert_eq!(names, vec!["REMOTE".to_string()]);
    }

    #[test]
    fn create_local_env_rejects_remote_source_variables() {
        let err = create_env_for_mcp_server(
            /*extra_env*/ None,
            &[McpServerEnvVar::Config {
                name: "REMOTE".to_string(),
                source: Some("remote".to_string()),
            }],
        )
        .expect_err("remote source should require remote stdio");

        assert!(
            err.to_string().contains("requires remote MCP stdio"),
            "unexpected error: {err}"
        );
    }

    fn write_headers_helper_script(dir: &Path, name: &str, output: &str) -> PathBuf {
        let script_path = dir.join(helper_script_name(name));
        std::fs::write(&script_path, helper_script_contents(output))
            .expect("headers helper script should be written");
        make_executable(&script_path);
        script_path
    }

    fn write_failing_headers_helper_script(dir: &Path, name: &str, stderr_output: &str) -> PathBuf {
        let script_path = dir.join(helper_script_name(name));
        std::fs::write(&script_path, failing_helper_script_contents(stderr_output))
            .expect("headers helper script should be written");
        make_executable(&script_path);
        script_path
    }

    #[cfg(unix)]
    fn helper_script_name(name: &str) -> String {
        name.to_string()
    }

    #[cfg(windows)]
    fn helper_script_name(name: &str) -> String {
        format!("{name}.cmd")
    }

    #[cfg(unix)]
    fn helper_script_contents(output: &str) -> String {
        format!("#!/bin/sh\nprintf '%s\\n' '{output}'\n")
    }

    #[cfg(unix)]
    fn failing_helper_script_contents(stderr_output: &str) -> String {
        format!("#!/bin/sh\nprintf '%s\\n' '{stderr_output}' >&2\nexit 1\n")
    }

    #[cfg(windows)]
    fn helper_script_contents(output: &str) -> String {
        format!("@echo off\r\necho {output}\r\n")
    }

    #[cfg(windows)]
    fn failing_helper_script_contents(stderr_output: &str) -> String {
        format!("@echo off\r\necho {stderr_output} 1>&2\r\nexit /b 1\r\n")
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path)
            .expect("headers helper script should have metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions)
            .expect("headers helper script should be executable");
    }

    #[cfg(windows)]
    fn make_executable(_path: &Path) {}

    #[tokio::test]
    #[serial(extra_rmcp_env)]
    async fn build_default_headers_runs_helper_and_allows_helper_to_override() {
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let script_path = write_headers_helper_script(
            temp_dir.path(),
            "headers-helper",
            r#"{"Authorization":"Bearer helper","X-Static":"helper","X-Arg":"from-helper"}"#,
        );
        let _guard = EnvVarGuard::set("CODEX_RMCP_CLIENT_HEADER_TEST", "from-env");

        let headers = build_default_headers(
            Some(HashMap::from([
                ("Authorization".to_string(), "Bearer static".to_string()),
                ("X-Static".to_string(), "static".to_string()),
            ])),
            Some(HashMap::from([(
                "X-Env".to_string(),
                "CODEX_RMCP_CLIENT_HEADER_TEST".to_string(),
            )])),
            Some(McpServerHttpHeadersHelperConfig::new(
                script_path.display().to_string(),
                Vec::new(),
            )),
        )
        .await
        .expect("headers should build");

        assert_eq!(
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer helper")
        );
        assert_eq!(
            headers
                .get("x-static")
                .and_then(|value| value.to_str().ok()),
            Some("helper")
        );
        assert_eq!(
            headers.get("x-env").and_then(|value| value.to_str().ok()),
            Some("from-env")
        );
        assert_eq!(
            headers.get("x-arg").and_then(|value| value.to_str().ok()),
            Some("from-helper")
        );
    }

    #[tokio::test]
    async fn build_default_headers_resolves_relative_helper_against_cwd() {
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        write_headers_helper_script(
            temp_dir.path(),
            "headers-helper",
            r#"{"Authorization":"Bearer helper"}"#,
        );
        let helper_program = format!(
            ".{}{}",
            std::path::MAIN_SEPARATOR,
            helper_script_name("headers-helper")
        );
        let mut helper = McpServerHttpHeadersHelperConfig::new(helper_program, Vec::new());
        helper.cwd = AbsolutePathBuf::from_absolute_path(temp_dir.path())
            .expect("tempdir path should be absolute");

        let headers = build_default_headers(
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            Some(helper),
        )
        .await
        .expect("headers should build");

        assert_eq!(
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer helper")
        );
    }

    #[tokio::test]
    async fn build_default_headers_rejects_invalid_helper_output() {
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let script_path =
            write_headers_helper_script(temp_dir.path(), "bad-headers-helper", "not-json");

        let err = build_default_headers(
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            Some(McpServerHttpHeadersHelperConfig::new(
                script_path.display().to_string(),
                Vec::new(),
            )),
        )
        .await
        .expect_err("invalid helper output should fail");

        assert!(
            err.to_string()
                .contains("must output a JSON object with string values"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn build_default_headers_rejects_oversized_helper_output() {
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let output = "x".repeat(MAX_HTTP_HEADERS_HELPER_STDOUT_BYTES + 1);
        let script_path =
            write_headers_helper_script(temp_dir.path(), "oversized-headers-helper", &output);

        let err = build_default_headers(
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            Some(McpServerHttpHeadersHelperConfig::new(
                script_path.display().to_string(),
                Vec::new(),
            )),
        )
        .await
        .expect_err("oversized helper output should fail");

        assert!(
            err.to_string().contains("wrote more than"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn build_default_headers_omits_helper_stderr_from_error() {
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let script_path = write_failing_headers_helper_script(
            temp_dir.path(),
            "failing-headers-helper",
            "secret-token-value",
        );

        let err = build_default_headers(
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            Some(McpServerHttpHeadersHelperConfig::new(
                script_path.display().to_string(),
                Vec::new(),
            )),
        )
        .await
        .expect_err("failing helper should fail");
        let message = err.to_string();

        assert!(
            message.contains("stderr omitted"),
            "unexpected error: {err}"
        );
        assert!(
            !message.contains("secret-token-value"),
            "stderr leaked in error: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    #[serial(extra_rmcp_env)]
    fn create_env_preserves_path_when_it_is_not_utf8() {
        use std::os::unix::ffi::OsStrExt;

        let raw_path = std::ffi::OsStr::from_bytes(b"/tmp/codex-\xFF/bin");
        let expected = raw_path.to_os_string();
        let _guard = EnvVarGuard::set("PATH", raw_path);

        let env =
            create_env_for_mcp_server(/*extra_env*/ None, &[]).expect("local MCP env should build");

        assert_eq!(env.get(OsStr::new("PATH")), Some(&expected));
    }
}
