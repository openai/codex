//! Implements the `codex doctor` diagnostic report.
//!
//! Doctor is intentionally read-mostly: checks inspect the current installation,
//! configuration, authentication, terminal, state paths, and bounded reachability
//! probes without attempting repair or starting long-lived services. Each check
//! returns a redacted, serializable row so the same data can back the human
//! summary and `--json` support report.
//!
//! A failing check should describe the problem and remediation, but it should not
//! mutate user state. That keeps the command safe to run before filing a support
//! issue or while diagnosing a broken local installation.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::io::IsTerminal;
use std::net::TcpStream;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use clap::Parser;
use codex_arg0::Arg0DispatchPaths;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_install_context::InstallContext;
use codex_install_context::StandalonePlatform;
use codex_login::CODEX_ACCESS_TOKEN_ENV_VAR;
use codex_login::CODEX_API_KEY_ENV_VAR;
use codex_login::OPENAI_API_KEY_ENV_VAR;
use codex_login::load_auth_dot_json;
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use codex_tui::Cli as TuiCli;
use codex_utils_cli::CliConfigOverrides;
use serde::Serialize;
use supports_color::Stream;

mod background;
mod output;
mod runtime;
mod updates;

use background::background_server_check;
use output::HumanOutputOptions;
use output::render_human_report;
use runtime::runtime_check;
use runtime::search_check;
use updates::updates_check;

/// Options for building a local Codex diagnostic report.
///
/// The command always runs the full bounded diagnostic set. Human output is
/// concise by default, while --verbose exposes local paths and command output
/// that are useful when debugging a specific installation.
#[derive(Debug, Parser)]
pub struct DoctorCommand {
    /// Emit a redacted machine-readable report.
    #[arg(long, default_value_t = false)]
    json: bool,

    /// Include extra local paths and command outputs in human output.
    #[arg(long, default_value_t = false)]
    verbose: bool,

    /// Disable ANSI color in human output.
    #[arg(long, default_value_t = false)]
    no_color: bool,

    /// Use ASCII status labels and separators in human output.
    #[arg(long, default_value_t = false)]
    ascii: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum CheckStatus {
    Ok,
    Warning,
    Fail,
    Skipped,
}

/// Machine-readable doctor output shared by human and JSON renderers.
///
/// The schema is intentionally flat: each check carries its own category,
/// status, details, remediation, and duration so support tooling can filter or
/// redact individual rows without understanding the renderer's section layout.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorReport {
    schema_version: u32,
    generated_at: String,
    overall_status: CheckStatus,
    codex_version: String,
    checks: Vec<DoctorCheck>,
}

/// One diagnostic result in the doctor report.
///
/// Summaries are safe for the default human view. Details may include local
/// paths or command output and are therefore shown only with --verbose in
/// human mode, while JSON consumers receive the full redacted report.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorCheck {
    id: String,
    category: String,
    status: CheckStatus,
    summary: String,
    details: Vec<String>,
    remediation: Option<String>,
    duration_ms: u64,
}

impl DoctorCheck {
    fn new(
        id: impl Into<String>,
        category: impl Into<String>,
        status: CheckStatus,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            category: category.into(),
            status,
            summary: summary.into(),
            details: Vec::new(),
            remediation: None,
            duration_ms: 0,
        }
    }

    fn detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }

    fn details(mut self, details: Vec<String>) -> Self {
        self.details.extend(details);
        self
    }

    fn remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }
}

/// Builds, renders, and exits according to the current doctor report.
///
/// This is the CLI entry point for codex doctor. It does not repair issues;
/// failures are represented in the report and cause a non-zero process exit so
/// scripts can distinguish a clean environment from one that needs attention.
pub async fn run_doctor(
    command: DoctorCommand,
    root_config_overrides: CliConfigOverrides,
    interactive: &TuiCli,
    arg0_paths: &Arg0DispatchPaths,
) -> anyhow::Result<()> {
    let report = build_report(&command, root_config_overrides, interactive, arg0_paths).await;

    if command.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!(
            "{}",
            render_human_report(&report, human_output_options(&command))
        );
    }

    if report.overall_status == CheckStatus::Fail {
        std::process::exit(1);
    }

    Ok(())
}

async fn build_report(
    command: &DoctorCommand,
    root_config_overrides: CliConfigOverrides,
    interactive: &TuiCli,
    arg0_paths: &Arg0DispatchPaths,
) -> DoctorReport {
    let mut checks = Vec::new();
    checks.push(timed_check(|| installation_check(command.verbose)));
    checks.push(timed_check(runtime_check));
    checks.push(timed_check(search_check));

    let config_result = load_config(root_config_overrides, interactive, arg0_paths).await;
    match &config_result {
        Ok(config) => {
            checks.push(timed_check(|| config_check(config)));
            checks.push(timed_check(|| auth_check(config)));
            checks.push(timed_check(|| updates_check(config)));
            checks.push(timed_check(network_check));
            checks.push(timed_check(|| mcp_check(config)));
            checks.push(timed_check(|| sandbox_check(config, arg0_paths)));
            checks.push(timed_check(terminal_check));
            checks.push(timed_check(|| state_check(config)));
            checks.push(timed_check(|| background_server_check(config)));
        }
        Err(err) => {
            checks.push(timed_check(|| {
                DoctorCheck::new(
                    "config.load",
                    "config",
                    CheckStatus::Fail,
                    "config could not be loaded",
                )
                .detail(err.to_string())
                .remediation("Fix the reported config error, then rerun codex doctor.")
            }));
            checks.push(timed_check(network_check));
            checks.push(timed_check(terminal_check));
            checks.push(timed_check(fallback_state_check));
        }
    }

    checks.push(timed_check(openai_reachability_check));

    let overall_status = overall_status(&checks);
    DoctorReport {
        schema_version: 1,
        generated_at: generated_at(),
        overall_status,
        codex_version: env!("CARGO_PKG_VERSION").to_string(),
        checks,
    }
}

async fn load_config(
    root_config_overrides: CliConfigOverrides,
    interactive: &TuiCli,
    arg0_paths: &Arg0DispatchPaths,
) -> anyhow::Result<Config> {
    let mut cli_kv_overrides = root_config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    if interactive.web_search {
        cli_kv_overrides.push((
            "web_search".to_string(),
            toml::Value::String("live".to_string()),
        ));
    }

    let overrides = ConfigOverrides {
        config_profile: interactive.config_profile.clone(),
        codex_self_exe: arg0_paths.codex_self_exe.clone(),
        codex_linux_sandbox_exe: arg0_paths.codex_linux_sandbox_exe.clone(),
        main_execve_wrapper_exe: arg0_paths.main_execve_wrapper_exe.clone(),
        ..Default::default()
    };

    Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, overrides)
        .await
        .context("failed to load Codex config")
}

fn timed_check(f: impl FnOnce() -> DoctorCheck) -> DoctorCheck {
    let start = Instant::now();
    let mut check = f();
    check.duration_ms = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    check
}

fn overall_status(checks: &[DoctorCheck]) -> CheckStatus {
    if checks.iter().any(|check| check.status == CheckStatus::Fail) {
        CheckStatus::Fail
    } else if checks
        .iter()
        .any(|check| check.status == CheckStatus::Warning)
    {
        CheckStatus::Warning
    } else {
        CheckStatus::Ok
    }
}

fn generated_at() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => {
            let seconds = duration.as_secs();
            format!("{seconds}s since unix epoch")
        }
        Err(_) => "unknown".to_string(),
    }
}

fn installation_check(verbose: bool) -> DoctorCheck {
    let mut details = Vec::new();
    let current_exe = env::current_exe().ok();
    push_path_detail(&mut details, "current executable", current_exe.as_deref());
    let inherited_managed_env = inherited_managed_env_for_cargo_binary(current_exe.as_deref());
    let install_context = doctor_install_context(current_exe.as_deref());
    details.push(format!(
        "install context: {}",
        describe_install_context(&install_context)
    ));
    if inherited_managed_env {
        details.push(
            "ignored inherited package-manager launch env for cargo-built binary".to_string(),
        );
    }
    details.push(format!(
        "managed by npm: {}",
        doctor_managed_by_npm(current_exe.as_deref())
    ));
    details.push(format!(
        "managed by bun: {}",
        env::var_os("CODEX_MANAGED_BY_BUN").is_some()
    ));
    push_env_path_detail(
        &mut details,
        "managed package root",
        "CODEX_MANAGED_PACKAGE_ROOT",
    );

    let path_entries = codex_path_entries();
    let mut status = CheckStatus::Ok;
    let mut summary = "installation looks consistent".to_string();
    let mut remediation = None;

    if path_entries.len() > 1 {
        details.push(format!("PATH codex entries: {}", path_entries.len()));
    }
    if verbose || path_entries.len() > 1 {
        details.extend(
            path_entries
                .iter()
                .enumerate()
                .map(|(index, path)| format!("PATH codex #{}: {path}", index + 1)),
        );
    }

    if doctor_managed_by_npm(current_exe.as_deref()) {
        match npm_global_root_check() {
            NpmRootCheck::Match { package_root } => {
                details.push(format!("npm update target: {}", package_root.display()));
            }
            NpmRootCheck::Mismatch {
                running_package_root,
                npm_package_root,
            } => {
                status = CheckStatus::Fail;
                summary =
                    "npm install -g @openai/codex would update a different install".to_string();
                remediation = Some(format!(
                    "Fix PATH or npm prefix so the running package root ({}) matches the npm global package root ({}).",
                    running_package_root.display(),
                    npm_package_root.display()
                ));
                details.push(format!(
                    "running package root: {}",
                    running_package_root.display()
                ));
                details.push(format!("npm package root: {}", npm_package_root.display()));
            }
            NpmRootCheck::MissingPackageRoot => {
                status = status.max(CheckStatus::Warning);
                summary = "npm-managed launch is missing package-root provenance".to_string();
                remediation = Some(
                    "Reinstall or update Codex so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT."
                        .to_string(),
                );
            }
            NpmRootCheck::NpmUnavailable(error) => {
                status = status.max(CheckStatus::Warning);
                summary = "npm-managed launch could not inspect npm global root".to_string();
                details.push(format!("npm root -g failed: {error}"));
            }
        }
    }

    let mut check = DoctorCheck::new("installation", "install", status, summary).details(details);
    if let Some(remediation) = remediation {
        check = check.remediation(remediation);
    }
    check
}

fn doctor_install_context(current_exe: Option<&Path>) -> InstallContext {
    if inherited_managed_env_for_cargo_binary(current_exe) {
        InstallContext::Other
    } else {
        InstallContext::current().clone()
    }
}

fn doctor_managed_by_npm(current_exe: Option<&Path>) -> bool {
    env::var_os("CODEX_MANAGED_BY_NPM").is_some()
        && !inherited_managed_env_for_cargo_binary(current_exe)
}

fn inherited_managed_env_for_cargo_binary(current_exe: Option<&Path>) -> bool {
    if env::var_os("CODEX_MANAGED_BY_NPM").is_none()
        && env::var_os("CODEX_MANAGED_BY_BUN").is_none()
    {
        return false;
    }

    let Some(current_exe) = current_exe else {
        return false;
    };
    let components = current_exe
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>();
    components
        .windows(2)
        .any(|window| window[0] == "target" && matches!(window[1].as_ref(), "debug" | "release"))
}

fn describe_install_context(context: &InstallContext) -> String {
    match context {
        InstallContext::Standalone {
            release_dir,
            resources_dir,
            platform,
        } => {
            let platform = match platform {
                StandalonePlatform::Unix => "unix",
                StandalonePlatform::Windows => "windows",
            };
            let resources = resources_dir
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string());
            format!(
                "standalone ({platform}, release {}, resources {resources})",
                release_dir.display()
            )
        }
        InstallContext::Npm => "npm".to_string(),
        InstallContext::Bun => "bun".to_string(),
        InstallContext::Brew => "brew".to_string(),
        InstallContext::Other => "other".to_string(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum NpmRootCheck {
    Match {
        package_root: PathBuf,
    },
    Mismatch {
        running_package_root: PathBuf,
        npm_package_root: PathBuf,
    },
    MissingPackageRoot,
    NpmUnavailable(String),
}

fn npm_global_root_check() -> NpmRootCheck {
    let Some(running_package_root) = env::var_os("CODEX_MANAGED_PACKAGE_ROOT").map(PathBuf::from)
    else {
        return NpmRootCheck::MissingPackageRoot;
    };

    let output = match run_command("npm", ["root", "-g"]) {
        Ok(output) => output,
        Err(err) => return NpmRootCheck::NpmUnavailable(err),
    };
    let Some(npm_root) = output.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return NpmRootCheck::NpmUnavailable("empty output from npm root -g".to_string());
    };

    compare_npm_package_roots(&running_package_root, &PathBuf::from(npm_root))
}

fn compare_npm_package_roots(running_package_root: &Path, npm_root: &Path) -> NpmRootCheck {
    let npm_package_root = npm_root.join("@openai").join("codex");
    let running = normalize_path_for_compare(running_package_root);
    let target = normalize_path_for_compare(&npm_package_root);
    if running == target {
        NpmRootCheck::Match {
            package_root: npm_package_root,
        }
    } else {
        NpmRootCheck::Mismatch {
            running_package_root: running_package_root.to_path_buf(),
            npm_package_root,
        }
    }
}

fn normalize_path_for_compare(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let raw = canonical.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        raw.to_ascii_lowercase()
    } else {
        raw
    }
}

fn codex_path_entries() -> Vec<String> {
    #[cfg(windows)]
    let result = run_command("where", ["codex"]);
    #[cfg(not(windows))]
    let result = run_command("which", ["-a", "codex"]);

    result
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn run_command<I, S>(program: &str, args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err(format!("exited with status {}", output.status));
        }
        return Err(stderr);
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn config_check(config: &Config) -> DoctorCheck {
    let mut details = Vec::new();
    details.push(format!("CODEX_HOME: {}", config.codex_home.display()));
    details.push(format!("cwd: {}", config.cwd.display()));
    details.push(format!(
        "model: {}",
        config.model.as_deref().unwrap_or("<default>")
    ));
    details.push(format!("model provider: {}", config.model_provider_id));
    details.push(format!("log dir: {}", config.log_dir.display()));
    details.push(format!("sqlite home: {}", config.sqlite_home.display()));
    details.push(format!("mcp servers: {}", config.mcp_servers.get().len()));
    config_toml_details(config, &mut details);

    let status = if config.startup_warnings.is_empty() {
        CheckStatus::Ok
    } else {
        details.extend(
            config
                .startup_warnings
                .iter()
                .map(|warning| format!("startup warning: {warning}")),
        );
        CheckStatus::Warning
    };

    DoctorCheck::new("config.load", "config", status, "config loaded").details(details)
}

fn config_toml_details(config: &Config, details: &mut Vec<String>) {
    let config_path = config.codex_home.join(codex_config::CONFIG_TOML_FILE);
    details.push(format!("config.toml: {}", config_path.display()));
    match std::fs::read_to_string(&config_path) {
        Ok(contents) => match toml::from_str::<toml::Value>(&contents) {
            Ok(_) => details.push("config.toml parse: ok".to_string()),
            Err(err) => details.push(format!("config.toml parse: {err}")),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            details.push("config.toml: missing".to_string());
        }
        Err(err) => details.push(format!("config.toml read: {err}")),
    }
}

fn auth_check(config: &Config) -> DoctorCheck {
    let mut details = Vec::new();
    let auth_path = config.codex_home.join("auth.json");
    details.push(format!(
        "auth storage mode: {:?}",
        config.cli_auth_credentials_store_mode
    ));
    details.push(format!("auth file: {}", auth_path.display()));

    let env_auth_vars = [
        OPENAI_API_KEY_ENV_VAR,
        CODEX_API_KEY_ENV_VAR,
        CODEX_ACCESS_TOKEN_ENV_VAR,
    ]
    .into_iter()
    .filter(|name| env_var_present(name))
    .collect::<Vec<_>>();
    if !env_auth_vars.is_empty() {
        details.push(format!(
            "auth env vars present: {}",
            env_auth_vars.join(", ")
        ));
    }

    match load_auth_dot_json(&config.codex_home, config.cli_auth_credentials_store_mode) {
        Ok(Some(auth)) => {
            details.push(format!("stored auth mode: {}", stored_auth_mode(&auth)));
            details.push(format!("stored API key: {}", auth.openai_api_key.is_some()));
            details.push(format!("stored ChatGPT tokens: {}", auth.tokens.is_some()));
            details.push(format!(
                "stored agent identity: {}",
                auth.agent_identity.is_some()
            ));
            let status = if env_auth_vars.len() > 1 {
                CheckStatus::Warning
            } else {
                CheckStatus::Ok
            };
            let summary = if status == CheckStatus::Warning {
                "auth is configured, but multiple auth env vars are present"
            } else {
                "auth is configured"
            };
            DoctorCheck::new("auth.credentials", "auth", status, summary).details(details)
        }
        Ok(None) if !env_auth_vars.is_empty() => DoctorCheck::new(
            "auth.credentials",
            "auth",
            CheckStatus::Ok,
            "auth is provided by environment",
        )
        .details(details),
        Ok(None) => DoctorCheck::new(
            "auth.credentials",
            "auth",
            CheckStatus::Fail,
            "no Codex credentials were found",
        )
        .details(details)
        .remediation("Run codex login or provide an API key through a supported auth env var."),
        Err(err) => DoctorCheck::new(
            "auth.credentials",
            "auth",
            CheckStatus::Fail,
            "stored credentials could not be read",
        )
        .detail(err.to_string())
        .remediation("Fix auth storage access or run codex login again."),
    }
}

fn stored_auth_mode(auth: &codex_login::AuthDotJson) -> &'static str {
    if let Some(mode) = auth.auth_mode {
        return match mode {
            codex_app_server_protocol::AuthMode::ApiKey => "api_key",
            codex_app_server_protocol::AuthMode::Chatgpt => "chatgpt",
            codex_app_server_protocol::AuthMode::ChatgptAuthTokens => "chatgpt_auth_tokens",
            codex_app_server_protocol::AuthMode::AgentIdentity => "agent_identity",
        };
    }
    if auth.openai_api_key.is_some() {
        "api_key"
    } else {
        "chatgpt"
    }
}

fn network_check() -> DoctorCheck {
    let mut details = Vec::new();
    let proxy_vars = [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "no_proxy",
    ];
    let present_proxy_vars = proxy_vars
        .into_iter()
        .filter(|name| env_var_present(name))
        .collect::<Vec<_>>();
    if present_proxy_vars.is_empty() {
        details.push("proxy env vars: none".to_string());
    } else {
        details.push(format!(
            "proxy env vars present: {}",
            present_proxy_vars.join(", ")
        ));
    }

    let mut status = CheckStatus::Ok;
    let mut summary = "network-related environment looks readable".to_string();
    for name in ["CODEX_CA_CERTIFICATE", "SSL_CERT_FILE"] {
        if let Some(raw) = env::var_os(name) {
            let path = PathBuf::from(raw);
            match std::fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() => {
                    details.push(format!("{name}: readable file {}", path.display()));
                }
                Ok(_) => {
                    status = CheckStatus::Warning;
                    summary = "custom CA env var does not point at a file".to_string();
                    details.push(format!("{name}: not a file {}", path.display()));
                }
                Err(err) => {
                    status = CheckStatus::Warning;
                    summary = "custom CA env var points at an unreadable path".to_string();
                    details.push(format!("{name}: {} ({err})", path.display()));
                }
            }
        }
    }

    DoctorCheck::new("network.env", "network", status, summary).details(details)
}

fn mcp_check(config: &Config) -> DoctorCheck {
    let servers = config.mcp_servers.get();
    if servers.is_empty() {
        return DoctorCheck::new(
            "mcp.config",
            "mcp",
            CheckStatus::Ok,
            "no MCP servers configured",
        );
    }

    let mut details = Vec::new();
    let mut transport_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut disabled = 0usize;
    let mut missing_env = Vec::new();
    let mut unreachable_http = Vec::new();

    for (name, server) in servers {
        if !server.enabled || server.disabled_reason.is_some() {
            disabled += 1;
        }
        match &server.transport {
            McpServerTransportConfig::Stdio {
                command,
                env,
                env_vars,
                cwd,
                ..
            } => {
                *transport_counts.entry("stdio").or_default() += 1;
                if let Some(cwd) = cwd
                    && !cwd.exists()
                {
                    missing_env.push(format!("{name}: cwd does not exist ({})", cwd.display()));
                }
                if command.trim().is_empty() {
                    missing_env.push(format!("{name}: stdio command is empty"));
                }
                if let Some(env) = env {
                    for key in env.keys().filter(|key| key.trim().is_empty()) {
                        missing_env.push(format!("{name}: empty env key {key}"));
                    }
                }
                for env_var in env_vars {
                    if !env_var.is_remote_source() && !env_var_present(env_var.name()) {
                        missing_env.push(format!("{name}: env var {} is not set", env_var.name()));
                    }
                }
            }
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var,
                env_http_headers,
                ..
            } => {
                *transport_counts.entry("streamable_http").or_default() += 1;
                if let Some(env_var) = bearer_token_env_var
                    && !env_var_present(env_var)
                {
                    missing_env.push(format!("{name}: bearer token env var {env_var} is not set"));
                }
                if let Some(headers) = env_http_headers {
                    for env_var in headers.values() {
                        if !env_var_present(env_var) {
                            missing_env
                                .push(format!("{name}: header env var {env_var} is not set"));
                        }
                    }
                }
                if let Err(err) = tcp_probe_url(url) {
                    unreachable_http.push(format!("{name}: {url} ({err})"));
                }
            }
        }
    }

    details.push(format!("configured servers: {}", servers.len()));
    details.push(format!("disabled servers: {disabled}"));
    for (transport, count) in transport_counts {
        details.push(format!("{transport} servers: {count}"));
    }
    details.extend(missing_env.iter().cloned());
    details.extend(
        unreachable_http
            .iter()
            .map(|detail| format!("reachability failed: {detail}")),
    );

    let required_missing = servers.iter().any(|(name, server)| {
        server.required
            && missing_env
                .iter()
                .any(|missing| missing.starts_with(&format!("{name}:")))
    });
    let status = if required_missing || !unreachable_http.is_empty() {
        CheckStatus::Fail
    } else if !missing_env.is_empty() {
        CheckStatus::Warning
    } else {
        CheckStatus::Ok
    };
    let summary = match status {
        CheckStatus::Ok => "MCP configuration is locally consistent",
        CheckStatus::Warning => "MCP configuration has missing optional inputs",
        CheckStatus::Fail => "MCP configuration has failing required inputs or reachability",
        CheckStatus::Skipped => unreachable!(),
    };

    let mut check = DoctorCheck::new("mcp.config", "mcp", status, summary).details(details);
    if status != CheckStatus::Ok {
        check = check.remediation("Set the missing MCP env vars or disable the affected server.");
    }
    check
}

fn sandbox_check(config: &Config, arg0_paths: &Arg0DispatchPaths) -> DoctorCheck {
    let mut details = Vec::new();
    details.push(format!(
        "approval policy: {:?}",
        config.permissions.approval_policy.value()
    ));
    let file_system_sandbox = config.permissions.file_system_sandbox_policy();
    details.push(format!("filesystem sandbox: {}", file_system_sandbox.kind));
    details.push(format!(
        "network sandbox: {}",
        config.permissions.network_sandbox_policy()
    ));
    push_path_detail(
        &mut details,
        "codex-linux-sandbox helper",
        arg0_paths.codex_linux_sandbox_exe.as_deref(),
    );
    push_path_detail(
        &mut details,
        "execve wrapper helper",
        arg0_paths.main_execve_wrapper_exe.as_deref(),
    );

    let mut status = CheckStatus::Ok;
    let mut summary = "sandbox configuration is readable".to_string();
    if let Some(helper) = arg0_paths.codex_linux_sandbox_exe.as_deref()
        && !helper.exists()
    {
        status = CheckStatus::Warning;
        summary = "Linux sandbox helper path does not exist".to_string();
    }

    DoctorCheck::new("sandbox.helpers", "sandbox", status, summary).details(details)
}

fn terminal_check() -> DoctorCheck {
    let info = terminal_info();
    let name = info.name;
    let mut details = vec![format!("terminal: {}", terminal_name(&info))];
    if let Some(term_program) = info.term_program {
        details.push(format!("TERM_PROGRAM: {term_program}"));
    }
    if let Some(version) = info.version {
        details.push(format!("terminal version: {version}"));
    }
    if let Some(term) = info.term {
        details.push(format!("TERM: {term}"));
    }
    if let Some(multiplexer) = info.multiplexer {
        details.push(format!("multiplexer: {}", multiplexer_name(&multiplexer)));
    }

    let status = if matches!(name, TerminalName::Dumb) {
        CheckStatus::Warning
    } else {
        CheckStatus::Ok
    };
    let summary = if status == CheckStatus::Warning {
        "terminal reports TERM=dumb"
    } else {
        "terminal metadata was detected"
    };
    DoctorCheck::new("terminal.env", "terminal", status, summary).details(details)
}

fn terminal_name(info: &TerminalInfo) -> &'static str {
    match info.name {
        TerminalName::AppleTerminal => "Apple Terminal",
        TerminalName::Ghostty => "Ghostty",
        TerminalName::Iterm2 => "iTerm2",
        TerminalName::WarpTerminal => "Warp",
        TerminalName::VsCode => "VS Code",
        TerminalName::WezTerm => "WezTerm",
        TerminalName::Kitty => "kitty",
        TerminalName::Alacritty => "Alacritty",
        TerminalName::Konsole => "Konsole",
        TerminalName::GnomeTerminal => "GNOME Terminal",
        TerminalName::Vte => "VTE",
        TerminalName::WindowsTerminal => "Windows Terminal",
        TerminalName::Dumb => "dumb",
        TerminalName::Unknown => "unknown",
    }
}

fn multiplexer_name(multiplexer: &Multiplexer) -> String {
    match multiplexer {
        Multiplexer::Tmux { version } => match version {
            Some(version) => format!("tmux {version}"),
            None => "tmux".to_string(),
        },
        Multiplexer::Zellij {} => "zellij".to_string(),
    }
}

fn state_check(config: &Config) -> DoctorCheck {
    let mut details = Vec::new();
    path_readiness(&mut details, "CODEX_HOME", &config.codex_home);
    path_readiness(&mut details, "log dir", &config.log_dir);
    path_readiness(&mut details, "sqlite home", &config.sqlite_home);
    let state_db = codex_state::state_db_path(&config.sqlite_home);
    path_readiness(&mut details, "state DB", &state_db);
    standalone_release_cache_details(&mut details);

    DoctorCheck::new(
        "state.paths",
        "state",
        CheckStatus::Ok,
        "state paths are inspectable",
    )
    .details(details)
}

fn fallback_state_check() -> DoctorCheck {
    let codex_home = find_codex_home();
    match codex_home {
        Ok(path) => DoctorCheck::new(
            "state.paths",
            "state",
            CheckStatus::Ok,
            "CODEX_HOME was resolved without config",
        )
        .detail(format!("CODEX_HOME: {}", path.display())),
        Err(err) => DoctorCheck::new(
            "state.paths",
            "state",
            CheckStatus::Warning,
            "CODEX_HOME could not be resolved",
        )
        .detail(err.to_string()),
    }
}

fn openai_reachability_check() -> DoctorCheck {
    let endpoints = [("api.openai.com", 443), ("chatgpt.com", 443)];
    let mut details = Vec::new();
    let mut failures = Vec::new();
    for (host, port) in endpoints {
        match tcp_probe_host(host, port) {
            Ok(()) => details.push(format!("{host}:{port}: reachable")),
            Err(err) => {
                details.push(format!("{host}:{port}: {err}"));
                failures.push(host);
            }
        }
    }

    let status = if failures.is_empty() {
        CheckStatus::Ok
    } else {
        CheckStatus::Fail
    };
    let summary = if failures.is_empty() {
        "OpenAI endpoints are reachable over TCP"
    } else {
        "one or more OpenAI endpoints are unreachable over TCP"
    };
    let mut check = DoctorCheck::new(
        "network.openai_reachability",
        "reachability",
        status,
        summary,
    )
    .details(details);
    if status != CheckStatus::Ok {
        check = check.remediation("Check proxy, VPN, firewall, DNS, and custom CA configuration.");
    }
    check
}

fn tcp_probe_url(url: &str) -> Result<(), String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| "URL is missing a scheme".to_string())?;
    let default_port = match scheme {
        "http" => 80,
        "https" => 443,
        _ => return Err(format!("unsupported scheme {scheme}")),
    };
    let authority = rest.split('/').next().unwrap_or(rest);
    let (host, port) = parse_host_port(authority, default_port)?;
    tcp_probe_host(&host, port)
}

fn parse_host_port(authority: &str, default_port: u16) -> Result<(String, u16), String> {
    if authority.is_empty() {
        return Err("URL host is empty".to_string());
    }
    if let Some((host, port)) = authority.rsplit_once(':')
        && let Ok(port) = port.parse::<u16>()
    {
        return Ok((host.trim_matches(['[', ']']).to_string(), port));
    }
    Ok((authority.trim_matches(['[', ']']).to_string(), default_port))
}

fn tcp_probe_host(host: &str, port: u16) -> Result<(), String> {
    let mut addrs = (host, port)
        .to_socket_addrs()
        .map_err(|err| format!("DNS lookup failed: {err}"))?;
    let Some(addr) = addrs.next() else {
        return Err("DNS lookup returned no addresses".to_string());
    };
    TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .map(|_| ())
        .map_err(|err| format!("connect failed: {err}"))
}

fn path_readiness(details: &mut Vec<String>, label: &str, path: &Path) {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let kind = if metadata.is_dir() {
                "dir"
            } else if metadata.is_file() {
                "file"
            } else {
                "other"
            };
            details.push(format!("{label}: {} ({kind})", path.display()));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            details.push(format!("{label}: {} (missing)", path.display()));
        }
        Err(err) => details.push(format!("{label}: {} ({err})", path.display())),
    }
}

fn standalone_release_cache_details(details: &mut Vec<String>) {
    let InstallContext::Standalone { release_dir, .. } = InstallContext::current() else {
        return;
    };
    let Some(releases_dir) = release_dir.parent() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(releases_dir) else {
        return;
    };
    let release_count = entries.filter_map(Result::ok).count();
    details.push(format!(
        "standalone release cache: {release_count} entries in {}",
        releases_dir.display()
    ));
}

fn push_path_detail(details: &mut Vec<String>, label: &str, path: Option<&Path>) {
    match path {
        Some(path) => details.push(format!("{label}: {}", path.display())),
        None => details.push(format!("{label}: none")),
    }
}

fn push_env_path_detail(details: &mut Vec<String>, label: &str, name: &str) {
    match env::var_os(name) {
        Some(path) => details.push(format!("{label}: {}", PathBuf::from(path).display())),
        None => details.push(format!("{label}: not set")),
    }
}

fn env_var_present(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn human_output_options(command: &DoctorCommand) -> HumanOutputOptions {
    let term = env::var("TERM").ok();
    let color_enabled = should_enable_color(
        command.no_color,
        env::var_os("NO_COLOR").is_some(),
        term.as_deref(),
        std::io::stdout().is_terminal(),
        supports_color::on(Stream::Stdout).is_some(),
    );
    HumanOutputOptions {
        verbose: command.verbose,
        ascii: command.ascii,
        color_enabled,
    }
}

fn should_enable_color(
    no_color_flag: bool,
    no_color_env: bool,
    term: Option<&str>,
    stdout_is_tty: bool,
    stream_supports_color: bool,
) -> bool {
    !no_color_flag
        && !no_color_env
        && term != Some("dumb")
        && stdout_is_tty
        && stream_supports_color
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overall_status_prefers_fail() {
        let checks = vec![
            DoctorCheck::new("a", "config", CheckStatus::Warning, "warning"),
            DoctorCheck::new("b", "auth", CheckStatus::Fail, "fail"),
        ];
        assert_eq!(overall_status(&checks), CheckStatus::Fail);
    }

    #[test]
    fn compare_npm_package_roots_detects_match() {
        let running = PathBuf::from("/prefix/lib/node_modules/@openai/codex");
        let npm_root = PathBuf::from("/prefix/lib/node_modules");
        assert_eq!(
            compare_npm_package_roots(&running, &npm_root),
            NpmRootCheck::Match {
                package_root: npm_root.join("@openai").join("codex")
            }
        );
    }

    #[test]
    fn compare_npm_package_roots_detects_mismatch() {
        let running = PathBuf::from("/old/lib/node_modules/@openai/codex");
        let npm_root = PathBuf::from("/new/lib/node_modules");
        assert_eq!(
            compare_npm_package_roots(&running, &npm_root),
            NpmRootCheck::Mismatch {
                running_package_root: running,
                npm_package_root: npm_root.join("@openai").join("codex"),
            }
        );
    }

    #[test]
    fn should_enable_color_respects_terminal_inputs() {
        assert!(should_enable_color(
            /*no_color_flag*/ false,
            /*no_color_env*/ false,
            Some("xterm-256color"),
            /*stdout_is_tty*/ true,
            /*stream_supports_color*/ true,
        ));
        assert!(!should_enable_color(
            /*no_color_flag*/ true,
            /*no_color_env*/ false,
            Some("xterm-256color"),
            /*stdout_is_tty*/ true,
            /*stream_supports_color*/ true,
        ));
        assert!(!should_enable_color(
            /*no_color_flag*/ false,
            /*no_color_env*/ true,
            Some("xterm-256color"),
            /*stdout_is_tty*/ true,
            /*stream_supports_color*/ true,
        ));
        assert!(!should_enable_color(
            /*no_color_flag*/ false,
            /*no_color_env*/ false,
            Some("dumb"),
            /*stdout_is_tty*/ true,
            /*stream_supports_color*/ true,
        ));
        assert!(!should_enable_color(
            /*no_color_flag*/ false,
            /*no_color_env*/ false,
            Some("xterm-256color"),
            /*stdout_is_tty*/ false,
            /*stream_supports_color*/ true,
        ));
    }
}
