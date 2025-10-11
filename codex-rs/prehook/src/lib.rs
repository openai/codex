use anyhow::Context as _;
use anyhow::Result;
use anyhow::anyhow;
use serde::Deserialize;
use serde::Serialize;
use std::io::IsTerminal;
// On Unix, `tokio::process::Command` exposes `pre_exec` directly; no trait import needed.
use std::path::PathBuf;
use tokio::process::Command;
use tokio::time::Duration;
use tokio::time::timeout;
use tracing::warn;
use uuid::Uuid;

/// What to do when the prehook backend errors (unreachable, bad JSON, timeout, etc.).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnErrorPolicy {
    Fail,
    Warn,
    Skip,
}

impl Default for OnErrorPolicy {
    fn default() -> Self {
        Self::Fail
    }
}

/// High-level action the caller intends to perform.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    Exec,
    ApplyPatch,
    Review,
    Schedule,
}

/// Execution/sandbox/model profile (subset; extend as needed).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EnvProfile {
    pub approval_policy: Option<String>,
    pub sandbox: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GitSummary {
    pub branch: Option<String>,
    pub dirty: bool,
    pub diff_stats: Option<String>,
}

/// Context sent to the prehook backends.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Context {
    pub id: String,
    pub timestamp_ms: i64,
    pub command_kind: CommandKind,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub os: String,
    pub user: Option<String>,
    pub env_profile: EnvProfile,
    pub git_summary: Option<GitSummary>,
    pub repo_root: Option<PathBuf>,
    pub relpath: Option<PathBuf>,
    pub task_metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub tty: Option<bool>,
    #[serde(default)]
    pub ci: Option<bool>,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub sanitized_env: std::collections::HashMap<String, String>,
}

impl Context {
    pub fn new(command_kind: CommandKind, cwd: PathBuf, args: Vec<String>) -> Self {
        Self {
            id: Uuid::now_v7().to_string(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            command_kind,
            args,
            cwd,
            os: std::env::consts::OS.to_string(),
            user: std::env::var("USER").ok(),
            env_profile: EnvProfile::default(),
            git_summary: None,
            repo_root: None,
            relpath: None,
            task_metadata: None,
            correlation_id: None,
            tty: Some(std::io::stdout().is_terminal()),
            ci: Some(std::env::var("CI").is_ok()),
            dry_run: Some(false),
            sanitized_env: default_sanitized_env(),
        }
    }
}

/// Decision returned by the prehook.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum Outcome {
    #[serde(rename = "allow")]
    Allow {
        message: Option<String>,
        ttl_ms: Option<u64>,
    },
    #[serde(rename = "deny")]
    Deny { reason: String },
    #[serde(rename = "ask")]
    Ask { message: String },
    #[serde(rename = "patch")]
    Patch {
        message: Option<String>,
        diff: String,
    },
    #[serde(rename = "augment")]
    Augment {
        message: Option<String>,
        context_items: Vec<serde_json::Value>,
    },
    #[serde(rename = "defer")]
    Defer {
        message: Option<String>,
        reason: Option<String>,
    },
    #[serde(rename = "rate_limit")]
    RateLimit {
        retry_after_ms: u64,
        #[serde(default)]
        message: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrehookConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_backend")]
    pub backend: String, // "mcp" | "script" | "chained"
    #[serde(default)]
    pub on_error: OnErrorPolicy,

    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub script: ScriptConfig,
}

fn default_backend() -> String {
    "mcp".to_string()
}

impl Default for PrehookConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: default_backend(),
            on_error: OnErrorPolicy::Fail,
            mcp: McpConfig::default(),
            script: ScriptConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpConfig {
    /// e.g. "stdio:/path/to/server" or "wss://host:port"
    pub server: Option<String>,
    /// Tool name to invoke, e.g. "codex.prehook.review"
    pub tool: Option<String>,
    /// Connect/startup timeout in ms (default 2000)
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    /// Tool call timeout in ms (default 5000)
    #[serde(default = "default_timeout_ms")]
    pub call_timeout_ms: u64,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            server: None,
            tool: Some("codex.prehook.review".to_string()),
            connect_timeout_ms: default_connect_timeout_ms(),
            call_timeout_ms: default_timeout_ms(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptConfig {
    pub cmd: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

impl Default for ScriptConfig {
    fn default() -> Self {
        Self {
            cmd: None,
            args: Vec::new(),
            timeout_ms: default_timeout_ms(),
        }
    }
}

fn default_timeout_ms() -> u64 {
    5_000
}
fn default_connect_timeout_ms() -> u64 {
    2_000
}

#[async_trait::async_trait]
pub trait PreHook: Send + Sync {
    async fn run(&self, ctx: &Context) -> Result<Outcome>;
}

pub struct ScriptPreHook {
    cfg: ScriptConfig,
    on_error: OnErrorPolicy,
}

impl ScriptPreHook {
    pub fn new(cfg: ScriptConfig, on_error: OnErrorPolicy) -> Self {
        Self { cfg, on_error }
    }
}

fn default_sanitized_env() -> std::collections::HashMap<String, String> {
    use regex_lite::Regex;
    let mut out = std::collections::HashMap::new();
    let allow = [
        "HOME", "LOGNAME", "PATH", "SHELL", "USER", "LANG", "LC_ALL", "TERM", "TMPDIR", "TZ",
        "PWD", "COLUMNS", "LINES",
    ];
    let deny = Regex::new(r"(?i)(TOKEN|SECRET|PASSWORD|WEBHOOK|API[_-]?KEY|ACCESS[_-]?KEY)")
        .unwrap_or_else(|_| Regex::new(r"(?!)").expect("fallback regex must compile"));
    for k in allow.iter() {
        if let Ok(v) = std::env::var(k)
            && !deny.is_match(k)
            && !deny.is_match(&v)
        {
            out.insert((*k).to_string(), v);
        }
    }
    for drop in [
        "GITHUB_TOKEN",
        "GH_TOKEN",
        "GITLAB_TOKEN",
        "OPENAI_API_KEY",
        "HUGGINGFACE_HUB_TOKEN",
        "HONEYCOMB_API_KEY",
        "SENTRY_DSN",
        "SLACK_WEBHOOK_URL",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "DATABASE_URL",
    ] {
        out.remove(drop);
    }
    out.retain(|k, _| !(k.starts_with("AWS_") || k.starts_with("SSH_") || k == "NPM_TOKEN"));
    out
}

#[async_trait::async_trait]
impl PreHook for ScriptPreHook {
    async fn run(&self, ctx: &Context) -> Result<Outcome> {
        let Some(cmd) = &self.cfg.cmd else {
            return Ok(Outcome::Allow {
                message: Some("prehook: script backend not configured".into()),
                ttl_ms: None,
            });
        };
        let payload = serde_json::to_vec(ctx)?;
        let mut cmd_builder = Command::new(cmd);
        cmd_builder
            .args(&self.cfg.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .env_clear()
            .envs(default_sanitized_env());
        #[cfg(unix)]
        unsafe {
            cmd_builder.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }
        let mut child = cmd_builder
            .spawn()
            .with_context(|| format!("failed to spawn prehook script {cmd}"))?;
        {
            use tokio::io::AsyncWriteExt;
            let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
            stdin.write_all(&payload).await?;
        }
        // script timeout
        let tmo = Duration::from_millis(self.cfg.timeout_ms.max(1));
        // We can't move `child` into wait_with_output then later kill it on timeout,
        // so split into wait() + collect output from pipes.
        let out = match timeout(tmo, child.wait()).await {
            Ok(Ok(status)) => {
                use tokio::io::AsyncReadExt;
                let mut stdout = Vec::new();
                if let Some(mut so) = child.stdout.take() {
                    let _ = so.read_to_end(&mut stdout).await;
                }
                std::process::Output {
                    status,
                    stdout,
                    stderr: Vec::new(),
                }
            }
            Ok(Err(e)) => return self.handle_err(anyhow!(e)),
            Err(_) => {
                #[cfg(unix)]
                if let Some(pid) = child.id() {
                    unsafe {
                        libc::kill(-(pid as i32), libc::SIGKILL);
                    }
                }
                let _ = child.kill().await;
                return self.handle_err(anyhow!("prehook script timed out"));
            }
        };
        if !out.status.success() {
            return self.handle_err(anyhow!("prehook script exit status {}", out.status));
        }
        if out.stdout.len() > 64 * 1024 {
            return self.handle_err(anyhow!("prehook script output too large"));
        }
        let parsed: Outcome = serde_json::from_slice(&out.stdout)
            .map_err(|e| anyhow!("invalid prehook JSON: {e}"))?;
        // Basic caps/validation
        match &parsed {
            Outcome::RateLimit { retry_after_ms, .. } if *retry_after_ms > 300_000 => {
                return self
                    .handle_err(anyhow!("rate_limit.retry_after_ms too large (>300000 ms)"));
            }
            Outcome::Augment { context_items, .. } if context_items.len() > 128 => {
                return self.handle_err(anyhow!("augment.context_items too long (>128)"));
            }
            _ => {}
        }
        Ok(parsed)
    }
}

impl ScriptPreHook {
    fn handle_err<T>(&self, err: anyhow::Error) -> Result<T> {
        match self.on_error {
            OnErrorPolicy::Fail => Err(err),
            OnErrorPolicy::Warn => {
                warn!("prehook(script) error: {err:#}");
                Err(err)
            }
            OnErrorPolicy::Skip => {
                warn!("prehook(script) skipped due to error: {err:#}");
                Err(err)
            }
        }
    }
}

pub struct McpPreHook {
    cfg: McpConfig,
    on_error: OnErrorPolicy,
}

impl McpPreHook {
    pub fn new(cfg: McpConfig, on_error: OnErrorPolicy) -> Self {
        Self { cfg, on_error }
    }
}

#[async_trait::async_trait]
impl PreHook for McpPreHook {
    async fn run(&self, ctx: &Context) -> Result<Outcome> {
        use mcp_types::ClientCapabilities;
        use mcp_types::Implementation;
        use mcp_types::InitializeRequestParams;
        let server = self
            .cfg
            .server
            .clone()
            .ok_or_else(|| anyhow!("prehook MCP server not configured"))?;
        let tool = self
            .cfg
            .tool
            .clone()
            .unwrap_or_else(|| "codex.prehook.review".to_string());
        // timeouts handled via connect_tmo/call_tmo below

        let payload = serde_json::to_value(ctx)?;
        let connect_tmo = Duration::from_millis(self.cfg.connect_timeout_ms.max(1));
        let call_tmo = Duration::from_millis(self.cfg.call_timeout_ms.max(1));
        let fut = async move {
            // Only stdio: scheme supported in MVP
            let (prog, args): (std::ffi::OsString, Vec<std::ffi::OsString>) =
                if let Some(path) = server.strip_prefix("stdio:") {
                    let parts: Vec<&str> = path.split_whitespace().collect();
                    let p = std::ffi::OsString::from(
                        parts
                            .first()
                            .ok_or_else(|| anyhow!("invalid stdio: path"))?,
                    );
                    let a = parts[1..].iter().map(std::ffi::OsString::from).collect();
                    (p, a)
                } else {
                    return Err(anyhow!("unsupported MCP server scheme (expected stdio:)"));
                };

            let client = tokio::time::timeout(
                connect_tmo,
                codex_mcp_client::McpClient::new_stdio_client(prog, args, None),
            )
            .await
            .map_err(|_| anyhow!("MCP connect timeout"))??;
            let init_params = InitializeRequestParams {
                capabilities: ClientCapabilities {
                    elicitation: None,
                    experimental: None,
                    roots: None,
                    sampling: None,
                },
                client_info: Implementation {
                    name: "codex-prehook".into(),
                    title: Some("Codex PreHook".into()),
                    version: env!("CARGO_PKG_VERSION").into(),
                    user_agent: None,
                },
                protocol_version: "2025-06-18".into(),
            };
            let _ = client.initialize(init_params, Some(call_tmo)).await?;
            let res = client
                .call_tool(tool, Some(payload), Some(call_tmo))
                .await?;
            if let Some(v) = res.structured_content {
                let outcome: Outcome = serde_json::from_value(v)
                    .map_err(|e| anyhow!("invalid MCP outcome JSON: {e}"))?;
                match &outcome {
                    Outcome::RateLimit { retry_after_ms, .. } if *retry_after_ms > 300_000 => {
                        return Err(anyhow!("rate_limit.retry_after_ms too large (>300000 ms)"));
                    }
                    Outcome::Augment { context_items, .. } if context_items.len() > 128 => {
                        return Err(anyhow!("augment.context_items too long (>128)"));
                    }
                    _ => {}
                }
                return Ok(outcome);
            }
            // Fallback: try to parse the first text content block as JSON
            for block in res.content {
                if let mcp_types::ContentBlock::TextContent(tc) = block
                    && tc.text.len() <= 64 * 1024
                    && let Ok(outcome) = serde_json::from_str::<Outcome>(&tc.text)
                {
                    match &outcome {
                        Outcome::RateLimit { retry_after_ms, .. } if *retry_after_ms > 300_000 => {
                            return Err(anyhow!(
                                "rate_limit.retry_after_ms too large (>300000 ms)"
                            ));
                        }
                        Outcome::Augment { context_items, .. } if context_items.len() > 128 => {
                            return Err(anyhow!("augment.context_items too long (>128)"));
                        }
                        _ => {}
                    }
                    warn!(
                        "prehook(mcp): parsed Outcome from text block fallback; prefer structured_content JSON"
                    );
                    return Ok(outcome);
                }
            }
            Err(anyhow!("MCP tool did not return structured_content JSON"))
        };

        match timeout(call_tmo, fut).await {
            Ok(Ok(outcome)) => Ok(outcome),
            Ok(Err(e)) => self.handle_err(e),
            Err(_) => self.handle_err(anyhow!("prehook MCP timed out")),
        }
    }
}

impl McpPreHook {
    fn handle_err<T>(&self, err: anyhow::Error) -> Result<T> {
        match self.on_error {
            OnErrorPolicy::Fail => Err(err),
            OnErrorPolicy::Warn => {
                warn!("prehook(mcp) error: {err:#}");
                Err(err)
            }
            OnErrorPolicy::Skip => {
                warn!("prehook(mcp) skipped due to error: {err:#}");
                Err(err)
            }
        }
    }
}

/// Helper to choose backend based on config.
pub struct PreHookDispatcher {
    cfg: PrehookConfig,
}

impl PreHookDispatcher {
    pub fn new(cfg: PrehookConfig) -> Self {
        Self { cfg }
    }

    pub async fn run(&self, ctx: &Context) -> Result<Outcome> {
        if !self.cfg.enabled {
            return Ok(Outcome::Allow {
                message: Some("prehook disabled".into()),
                ttl_ms: None,
            });
        }
        match self.cfg.backend.as_str() {
            "script" => {
                let b = ScriptPreHook::new(self.cfg.script.clone(), self.cfg.on_error);
                b.run(ctx).await
            }
            // For MVP, "chained" is equivalent to MCP-only unless MCP fails; then we try script.
            "chained" => {
                let mcp = McpPreHook::new(self.cfg.mcp.clone(), self.cfg.on_error);
                match mcp.run(ctx).await {
                    Ok(Outcome::Defer { .. }) => {
                        let b = ScriptPreHook::new(self.cfg.script.clone(), self.cfg.on_error);
                        b.run(ctx).await
                    }
                    Ok(o) => Ok(o),
                    Err(e) => {
                        warn!("prehook chained: MCP failed, trying script: {e:#}");
                        let b = ScriptPreHook::new(self.cfg.script.clone(), self.cfg.on_error);
                        b.run(ctx).await
                    }
                }
            }
            _ => {
                let mcp = McpPreHook::new(self.cfg.mcp.clone(), self.cfg.on_error);
                mcp.run(ctx).await
            }
        }
    }
}

// --- Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io::Write;

    #[tokio::test]
    async fn script_prehook_allows_when_not_configured() {
        let hook = ScriptPreHook::new(
            ScriptConfig {
                cmd: None,
                args: vec![],
                timeout_ms: 100,
            },
            OnErrorPolicy::Fail,
        );
        let ctx = Context::new(
            CommandKind::Exec,
            PathBuf::from("/"),
            vec!["echo".to_string()],
        );
        let out = hook.run(&ctx).await.unwrap();
        match out {
            Outcome::Allow { .. } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn script_prehook_times_out() {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("sleep.sh");
        let mut f = std::fs::File::create(&script_path).unwrap();
        writeln!(f, "#!/usr/bin/env bash").unwrap();
        writeln!(f, "sleep 2").unwrap();
        drop(f);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = std::fs::metadata(&script_path).unwrap().permissions();
            p.set_mode(0o755);
            std::fs::set_permissions(&script_path, p).unwrap();
        }
        let cfg = ScriptConfig {
            cmd: Some(script_path.to_string_lossy().to_string()),
            args: vec![],
            timeout_ms: 100,
        };
        let hook = ScriptPreHook::new(cfg, OnErrorPolicy::Fail);
        let ctx = Context::new(CommandKind::Exec, PathBuf::from("/"), vec!["noop".into()]);
        let res = hook.run(&ctx).await;
        assert!(res.is_err(), "expected timeout error");
    }
    #[tokio::test]
    async fn script_prehook_parses_outcome() {
        // Create a tiny script that echoes a Deny JSON from stdin context.
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("hook.sh");
        let mut f = std::fs::File::create(&script_path).unwrap();
        writeln!(f, "#!/usr/bin/env bash").unwrap();
        // Use printf to avoid brace interpretation by Rust's format string.
        writeln!(f, "cat >/dev/null; printf '%s\n' '{{\"decision\":\"deny\",\"reason\":\"blocked by policy\"}}'").unwrap();
        drop(f);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = std::fs::metadata(&script_path).unwrap().permissions();
            p.set_mode(0o755);
            std::fs::set_permissions(&script_path, p).unwrap();
        }
        let cfg = ScriptConfig {
            cmd: Some(script_path.to_string_lossy().to_string()),
            args: vec![],
            timeout_ms: 1000,
        };
        let hook = ScriptPreHook::new(cfg, OnErrorPolicy::Fail);
        let ctx = Context::new(CommandKind::Exec, PathBuf::from("/"), vec![]);
        let out = hook.run(&ctx).await.unwrap();
        match out {
            Outcome::Deny { reason } => assert_eq!(reason, "blocked by policy"),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
