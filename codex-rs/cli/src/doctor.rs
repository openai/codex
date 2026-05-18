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
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::future::Future;
use std::io::IsTerminal;
use std::io::Read;
use std::net::IpAddr;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use clap::Parser;
use codex_api::ApiError;
use codex_api::ResponsesWebsocketClient;
use codex_api::is_azure_responses_provider;
use codex_arg0::Arg0DispatchPaths;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_features::FEATURES;
use codex_install_context::InstallContext;
use codex_install_context::StandalonePlatform;
use codex_login::AuthDotJson;
use codex_login::AuthManager;
use codex_login::CODEX_ACCESS_TOKEN_ENV_VAR;
use codex_login::CODEX_API_KEY_ENV_VAR;
use codex_login::CodexAuth;
use codex_login::OPENAI_API_KEY_ENV_VAR;
use codex_login::default_client::build_reqwest_client;
use codex_login::default_client::default_headers;
use codex_login::load_auth_dot_json;
use codex_model_provider::create_model_provider;
use codex_protocol::protocol::AskForApproval;
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use codex_tui::Cli as TuiCli;
use codex_utils_cli::CliConfigOverrides;
use http::HeaderMap;
use http::HeaderValue;
use serde::Serialize;
use supports_color::Stream;

mod background;
mod output;
mod progress;
mod runtime;
mod updates;

use background::background_server_check;
use output::HumanOutputOptions;
use output::redact_detail;
use output::render_human_report;
use progress::DoctorProgress;
use progress::doctor_progress;
use runtime::runtime_check;
use runtime::search_check;
use updates::updates_check;

const OPENAI_BETA_HEADER: &str = "OpenAI-Beta";
const RESPONSES_WEBSOCKETS_V2_BETA_HEADER_VALUE: &str = "responses_websockets=2026-02-06";
const WEBSOCKET_IMMEDIATE_CLOSE_GRACE: Duration = Duration::from_millis(250);
const SLOW_CHECK_PROGRESS_THRESHOLD: Duration = Duration::from_secs(2);
const SLOW_CHECK_PROGRESS_INTERVAL: Duration = Duration::from_secs(1);
const PROXY_ENV_VARS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
];
const COLOR_ENV_VARS: &[&str] = &[
    "COLORTERM",
    "NO_COLOR",
    "CLICOLOR",
    "CLICOLOR_FORCE",
    "FORCE_COLOR",
    "COLORFGBG",
];
const TERMINAL_DIMENSION_ENV_VARS: &[&str] = &["COLUMNS", "LINES"];
const TERMINFO_ENV_VARS: &[&str] = &["TERMINFO", "TERMINFO_DIRS"];
const LOCALE_ENV_VARS: &[&str] = &["LC_ALL", "LC_CTYPE", "LANG"];
#[cfg(windows)]
const NPM_COMMAND: &str = "npm.cmd";
#[cfg(not(windows))]
const NPM_COMMAND: &str = "npm";
const REMOTE_TERMINAL_ENV_VARS: &[&str] = &[
    "SSH_TTY",
    "SSH_CONNECTION",
    "SSH_CLIENT",
    "MOSH_IP",
    "WSL_DISTRO_NAME",
    "WSL_INTEROP",
    "VSCODE_INJECTION",
    "VSCODE_IPC_HOOK_CLI",
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "WT_SESSION",
];
const TMUX_OPTION_NAMES: &[&str] = &[
    "extended-keys",
    "xterm-keys",
    "allow-passthrough",
    "set-clipboard",
    "focus-events",
];
const NARROW_TERMINAL_COLUMNS: u16 = 80;
const NARROW_TERMINAL_ROWS: u16 = 24;

/// Options for building a local Codex diagnostic report.
///
/// The command always runs the full bounded diagnostic set. Human output includes
/// detailed diagnostics by default; --summary keeps the terminal output compact.
#[derive(Debug, Parser)]
pub struct DoctorCommand {
    /// Emit a redacted machine-readable report.
    #[arg(long, default_value_t = false)]
    json: bool,

    /// Only show grouped check rows and the final count summary.
    #[arg(long, default_value_t = false)]
    summary: bool,

    /// Expand long lists in detailed human output.
    #[arg(long, default_value_t = false)]
    all: bool,

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
}

/// Machine-readable doctor output shared by human and JSON renderers.
///
/// The schema is intentionally flat: each check carries its own category,
/// status, details, remediation, and duration so support tooling can filter or
/// redact individual rows without understanding the renderer's section layout.
#[derive(Clone, Debug, Serialize)]
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
/// Summaries are safe for compact human output. Details may include local paths
/// or command output and are redacted before rendering or JSON serialization.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorCheck {
    id: String,
    category: String,
    status: CheckStatus,
    summary: String,
    details: Vec<String>,
    issues: Vec<DoctorIssue>,
    remediation: Option<String>,
    duration_ms: u64,
}

/// Structured cause/remedy metadata for a non-ok doctor check.
///
/// Human output uses issues to make warnings and failures self-explanatory:
/// the row headline says what is wrong, matching detail rows show measured vs.
/// expected values, and remedies are printed as explicit next actions.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorIssue {
    severity: CheckStatus,
    cause: String,
    measured: Option<String>,
    expected: Option<String>,
    remedy: Option<String>,
    fields: Vec<String>,
}

impl DoctorIssue {
    fn new(severity: CheckStatus, cause: impl Into<String>) -> Self {
        Self {
            severity,
            cause: cause.into(),
            measured: None,
            expected: None,
            remedy: None,
            fields: Vec::new(),
        }
    }

    fn measured(mut self, measured: impl Into<String>) -> Self {
        self.measured = Some(measured.into());
        self
    }

    fn expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    fn remedy(mut self, remedy: impl Into<String>) -> Self {
        self.remedy = Some(remedy.into());
        self
    }

    fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }
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
            issues: Vec::new(),
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

    fn issue(mut self, issue: DoctorIssue) -> Self {
        self.issues.push(issue);
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
        println!(
            "{}",
            serde_json::to_string_pretty(&redacted_json_report(&report))?
        );
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
    let progress = doctor_progress(command.json);
    let mut checks = Vec::new();
    checks.push(run_sync_check("installation", progress.clone(), || {
        installation_check(!command.summary)
    }));
    checks.push(run_sync_check("runtime", progress.clone(), runtime_check));
    checks.push(run_sync_check("search", progress.clone(), search_check));

    progress.begin("config");
    let config_result = load_config(root_config_overrides, interactive, arg0_paths).await;
    match &config_result {
        Ok(config) => {
            let auth_manager =
                AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ true).await;
            let reachability_plan = provider_reachability_plan(config);
            let (
                config_check,
                auth_check,
                updates_check,
                network_check,
                websocket_check,
                mcp_check,
                sandbox_check,
                terminal_check,
                state_check,
                background_server_check,
                reachability_check,
            ) = tokio::join!(
                async { run_sync_check("config", progress.clone(), || config_check(config)) },
                async { run_sync_check("auth", progress.clone(), || auth_check(config)) },
                async { run_sync_check("updates", progress.clone(), || updates_check(config)) },
                async { run_sync_check("network", progress.clone(), network_check) },
                run_async_check(
                    "websocket",
                    progress.clone(),
                    websocket_reachability_check(config, Some(auth_manager)),
                ),
                run_async_check("MCP", progress.clone(), mcp_check(config)),
                async {
                    run_sync_check("sandbox", progress.clone(), || {
                        sandbox_check(config, arg0_paths)
                    })
                },
                async {
                    run_sync_check("terminal", progress.clone(), || {
                        terminal_check(command.no_color)
                    })
                },
                run_async_check("state", progress.clone(), state_check(config)),
                async {
                    run_sync_check("app-server", progress.clone(), || {
                        background_server_check(config)
                    })
                },
                run_async_check(
                    "provider reachability",
                    progress.clone(),
                    provider_reachability_check(reachability_plan),
                ),
            );
            checks.extend([
                config_check,
                auth_check,
                updates_check,
                network_check,
                websocket_check,
                mcp_check,
                sandbox_check,
                terminal_check,
                state_check,
                background_server_check,
                reachability_check,
            ]);
        }
        Err(err) => {
            let reachability_plan = default_reachability_plan();
            let (config_check, network_check, terminal_check, state_check, reachability_check) = tokio::join!(
                async {
                    run_sync_check("config", progress.clone(), || {
                        DoctorCheck::new(
                            "config.load",
                            "config",
                            CheckStatus::Fail,
                            "config could not be loaded",
                        )
                        .detail(err.to_string())
                        .remediation("Fix the reported config error, then rerun codex doctor.")
                    })
                },
                async { run_sync_check("network", progress.clone(), network_check) },
                async {
                    run_sync_check("terminal", progress.clone(), || {
                        terminal_check(command.no_color)
                    })
                },
                async { run_sync_check("state", progress.clone(), fallback_state_check) },
                run_async_check(
                    "provider reachability",
                    progress.clone(),
                    provider_reachability_check(reachability_plan),
                ),
            );
            checks.extend([
                config_check,
                network_check,
                terminal_check,
                state_check,
                reachability_check,
            ]);
        }
    }

    progress.settle();

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
        ephemeral: Some(true),
        ..config_overrides_from_interactive(interactive, arg0_paths)
    };

    ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides)
        .harness_overrides(overrides)
        .build()
        .await
        .context("failed to load Codex config")
}

fn config_overrides_from_interactive(
    interactive: &TuiCli,
    arg0_paths: &Arg0DispatchPaths,
) -> ConfigOverrides {
    let approval_policy = if interactive.dangerously_bypass_approvals_and_sandbox {
        Some(AskForApproval::Never)
    } else {
        interactive.approval_policy.map(Into::into)
    };
    let sandbox_mode = if interactive.dangerously_bypass_approvals_and_sandbox {
        Some(codex_protocol::config_types::SandboxMode::DangerFullAccess)
    } else {
        interactive.sandbox_mode.map(Into::into)
    };
    ConfigOverrides {
        model: interactive.model.clone(),
        config_profile: interactive.config_profile.clone(),
        approval_policy,
        sandbox_mode,
        cwd: interactive.cwd.clone(),
        model_provider: interactive
            .oss
            .then(|| interactive.oss_provider.clone())
            .flatten(),
        codex_self_exe: arg0_paths.codex_self_exe.clone(),
        codex_linux_sandbox_exe: arg0_paths.codex_linux_sandbox_exe.clone(),
        main_execve_wrapper_exe: arg0_paths.main_execve_wrapper_exe.clone(),
        show_raw_agent_reasoning: interactive.oss.then_some(true),
        additional_writable_roots: interactive.add_dir.clone(),
        ..Default::default()
    }
}

/// JSON support report emitted by `codex doctor --json`.
///
/// The report is keyed by check id so support tooling can fetch paths like
/// `checks["terminal.metadata"]` without scanning arrays. Human rendering can
/// reorder or group rows independently, but this JSON shape should stay stable
/// across cosmetic output changes.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDoctorReport {
    schema_version: u32,
    generated_at: String,
    overall_status: CheckStatus,
    codex_version: String,
    checks: BTreeMap<String, JsonDoctorCheck>,
}

/// One redacted check in the JSON support report.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDoctorCheck {
    id: String,
    category: String,
    status: CheckStatus,
    summary: String,
    details: BTreeMap<String, JsonDetailValue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    issues: Vec<JsonDoctorIssue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
    remediation: Option<String>,
    duration_ms: u64,
}

/// One redacted issue in the JSON support report.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDoctorIssue {
    severity: CheckStatus,
    cause: String,
    measured: Option<String>,
    expected: Option<String>,
    remedy: Option<String>,
    fields: Vec<String>,
}

/// JSON detail value that preserves repeated detail keys without inventing names.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
enum JsonDetailValue {
    One(String),
    Many(Vec<String>),
}

impl JsonDetailValue {
    fn push(&mut self, value: String) {
        match self {
            JsonDetailValue::One(previous) => {
                *self = JsonDetailValue::Many(vec![std::mem::take(previous), value]);
            }
            JsonDetailValue::Many(values) => values.push(value),
        }
    }
}

fn redacted_json_report(report: &DoctorReport) -> JsonDoctorReport {
    let checks = report
        .checks
        .iter()
        .map(|check| {
            let json_check = redacted_json_check(check);
            (check.id.clone(), json_check)
        })
        .collect();
    JsonDoctorReport {
        schema_version: report.schema_version,
        generated_at: report.generated_at.clone(),
        overall_status: report.overall_status,
        codex_version: report.codex_version.clone(),
        checks,
    }
}

fn redacted_json_check(check: &DoctorCheck) -> JsonDoctorCheck {
    let (details, notes) = structured_json_details(&check.details);
    JsonDoctorCheck {
        id: check.id.clone(),
        category: check.category.clone(),
        status: check.status,
        summary: check.summary.clone(),
        details,
        issues: check.issues.iter().map(redacted_json_issue).collect(),
        notes,
        remediation: check.remediation.as_deref().map(redact_detail),
        duration_ms: check.duration_ms,
    }
}

fn redacted_json_issue(issue: &DoctorIssue) -> JsonDoctorIssue {
    JsonDoctorIssue {
        severity: issue.severity,
        cause: redact_detail(&issue.cause),
        measured: issue.measured.as_deref().map(redact_detail),
        expected: issue.expected.as_deref().map(redact_detail),
        remedy: issue.remedy.as_deref().map(redact_detail),
        fields: issue
            .fields
            .iter()
            .map(|field| redact_detail(field))
            .collect(),
    }
}

/// Converts redacted `label: value` detail strings into JSON key/value fields.
///
/// Detail strings that do not follow the doctor detail convention are preserved
/// as notes instead of being dropped. Repeated labels become arrays so callers
/// can still retrieve the common scalar case directly while keeping all values.
fn structured_json_details(details: &[String]) -> (BTreeMap<String, JsonDetailValue>, Vec<String>) {
    let mut structured: BTreeMap<String, JsonDetailValue> = BTreeMap::new();
    let mut notes = Vec::new();
    for detail in details {
        let redacted = redact_detail(detail);
        let Some((key, value)) = redacted.split_once(": ") else {
            notes.push(redacted);
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            notes.push(redacted);
            continue;
        }
        let value = value.to_string();
        match structured.get_mut(key) {
            Some(existing) => existing.push(value),
            None => {
                structured.insert(key.to_string(), JsonDetailValue::One(value));
            }
        }
    }
    (structured, notes)
}

fn run_sync_check(
    label: &'static str,
    progress: Arc<dyn DoctorProgress>,
    f: impl FnOnce() -> DoctorCheck,
) -> DoctorCheck {
    progress.begin(label);
    let start = Instant::now();
    let mut check = f();
    check.duration_ms = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    progress.finish(label, check.status);
    check
}

async fn run_async_check<Fut>(
    label: &'static str,
    progress: Arc<dyn DoctorProgress>,
    future: Fut,
) -> DoctorCheck
where
    Fut: Future<Output = DoctorCheck>,
{
    progress.begin(label);
    let start = Instant::now();
    tokio::pin!(future);
    let mut progress_interval = tokio::time::interval(SLOW_CHECK_PROGRESS_INTERVAL);
    loop {
        tokio::select! {
            mut check = &mut future => {
                check.duration_ms = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
                progress.finish(label, check.status);
                return check;
            }
            _ = progress_interval.tick() => {
                let elapsed = start.elapsed();
                if elapsed >= SLOW_CHECK_PROGRESS_THRESHOLD {
                    progress.heartbeat(label, elapsed);
                }
            }
        }
    }
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

fn installation_check(show_details: bool) -> DoctorCheck {
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
    if show_details || path_entries.len() > 1 {
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

    let output = match run_command(NPM_COMMAND, ["root", "-g"]) {
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

fn display_list<T: AsRef<str>>(items: &[T]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>()
            .join(", ")
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
    feature_flag_details(config, &mut details);
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

fn feature_flag_details(config: &Config, details: &mut Vec<String>) {
    let features = config.features.get();
    let enabled_features = FEATURES
        .iter()
        .filter(|spec| features.enabled(spec.id))
        .map(|spec| spec.key)
        .collect::<Vec<_>>();
    let overrides = FEATURES
        .iter()
        .filter(|spec| features.enabled(spec.id) != spec.default_enabled)
        .map(|spec| format!("{}={}", spec.key, features.enabled(spec.id)))
        .collect::<Vec<_>>();
    details.push(format!("feature flags enabled: {}", enabled_features.len()));
    details.push(format!(
        "enabled feature flags: {}",
        display_list(&enabled_features)
    ));
    details.push(format!(
        "feature flag overrides: {}",
        display_list(&overrides)
    ));
    for usage in features.legacy_feature_usages() {
        details.push(format!(
            "legacy feature flag: {} -> {}",
            usage.alias,
            usage.feature.key()
        ));
    }
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
    if let Some(check) = provider_specific_auth_check(
        config.model_provider.requires_openai_auth,
        config.model_provider.env_key.as_deref(),
        config.model_provider.env_key_instructions.as_deref(),
        details.clone(),
        env_var_present,
    ) {
        return check;
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
            let auth_issues = stored_auth_issues(&auth, env_var_present);
            details.extend(
                auth_issues
                    .iter()
                    .map(|issue| format!("stored auth issue: {issue}")),
            );
            let status = if !auth_issues.is_empty() && env_auth_vars.is_empty() {
                CheckStatus::Fail
            } else if !auth_issues.is_empty() || env_auth_vars.len() > 1 {
                CheckStatus::Warning
            } else {
                CheckStatus::Ok
            };
            let summary = match status {
                CheckStatus::Ok => "auth is configured",
                CheckStatus::Warning if !auth_issues.is_empty() => {
                    "auth is provided by environment, but stored credentials are incomplete"
                }
                CheckStatus::Warning => {
                    "auth is configured, but multiple auth env vars are present"
                }
                CheckStatus::Fail => "stored credentials are incomplete",
            };
            let mut check =
                DoctorCheck::new("auth.credentials", "auth", status, summary).details(details);
            if status == CheckStatus::Fail {
                check =
                    check.remediation("Run codex login again or provide a supported auth env var.");
            }
            check
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

fn provider_specific_auth_check(
    requires_openai_auth: bool,
    provider_env_key: Option<&str>,
    provider_env_key_instructions: Option<&str>,
    mut details: Vec<String>,
    env_var_present: impl Fn(&str) -> bool,
) -> Option<DoctorCheck> {
    details.push(format!(
        "model provider requires OpenAI auth: {requires_openai_auth}"
    ));
    if requires_openai_auth {
        return None;
    }

    match provider_env_key {
        Some(env_key) if env_var_present(env_key) => {
            details.push(format!("provider auth env var: {env_key} (present)"));
            Some(
                DoctorCheck::new(
                    "auth.credentials",
                    "auth",
                    CheckStatus::Ok,
                    "auth is provided by the active model provider",
                )
                .details(details),
            )
        }
        Some(env_key) => {
            details.push(format!("provider auth env var: {env_key} (missing)"));
            let remediation = provider_env_key_instructions
                .map(str::to_string)
                .unwrap_or_else(|| format!("Set {env_key} for the active model provider."));
            Some(
                DoctorCheck::new(
                    "auth.credentials",
                    "auth",
                    CheckStatus::Fail,
                    "active model provider auth env var is missing",
                )
                .details(details)
                .remediation(remediation),
            )
        }
        None => Some(
            DoctorCheck::new(
                "auth.credentials",
                "auth",
                CheckStatus::Ok,
                "OpenAI auth is not required for the active model provider",
            )
            .details(details),
        ),
    }
}

fn stored_auth_mode(auth: &codex_login::AuthDotJson) -> &'static str {
    match stored_auth_mode_value(auth) {
        codex_app_server_protocol::AuthMode::ApiKey => "api_key",
        codex_app_server_protocol::AuthMode::Chatgpt => "chatgpt",
        codex_app_server_protocol::AuthMode::ChatgptAuthTokens => "chatgpt_auth_tokens",
        codex_app_server_protocol::AuthMode::AgentIdentity => "agent_identity",
    }
}

fn stored_auth_mode_value(auth: &AuthDotJson) -> codex_app_server_protocol::AuthMode {
    if let Some(mode) = auth.auth_mode {
        return mode;
    }
    if auth.openai_api_key.is_some() {
        codex_app_server_protocol::AuthMode::ApiKey
    } else {
        codex_app_server_protocol::AuthMode::Chatgpt
    }
}

fn stored_auth_issues(
    auth: &AuthDotJson,
    env_var_present: impl Fn(&str) -> bool,
) -> Vec<&'static str> {
    let mut issues = Vec::new();
    match stored_auth_mode_value(auth) {
        codex_app_server_protocol::AuthMode::ApiKey => {
            let stored_key_present = auth
                .openai_api_key
                .as_deref()
                .is_some_and(|key| !key.trim().is_empty());
            let env_key_present =
                env_var_present(OPENAI_API_KEY_ENV_VAR) || env_var_present(CODEX_API_KEY_ENV_VAR);
            if !stored_key_present && !env_key_present {
                issues.push("API key auth is missing an API key");
            }
        }
        codex_app_server_protocol::AuthMode::Chatgpt => {
            match auth.tokens.as_ref() {
                Some(tokens) => {
                    if tokens.access_token.trim().is_empty() {
                        issues.push("ChatGPT auth is missing an access token");
                    }
                    if tokens.refresh_token.trim().is_empty() {
                        issues.push("ChatGPT auth is missing a refresh token");
                    }
                }
                None => issues.push("ChatGPT auth is missing token data"),
            }
            if auth.last_refresh.is_none() {
                issues.push("ChatGPT auth is missing refresh metadata");
            }
        }
        codex_app_server_protocol::AuthMode::ChatgptAuthTokens => {
            match auth.tokens.as_ref() {
                Some(tokens) => {
                    if tokens.access_token.trim().is_empty() {
                        issues.push("external ChatGPT auth is missing an access token");
                    }
                    if tokens.account_id.is_none() && tokens.id_token.chatgpt_account_id.is_none() {
                        issues.push("external ChatGPT auth is missing a ChatGPT account id");
                    }
                }
                None => issues.push("external ChatGPT auth is missing token data"),
            }
            if auth.last_refresh.is_none() {
                issues.push("external ChatGPT auth is missing refresh metadata");
            }
        }
        codex_app_server_protocol::AuthMode::AgentIdentity => {
            if auth
                .agent_identity
                .as_deref()
                .is_none_or(|token| token.trim().is_empty())
            {
                issues.push("agent identity auth is missing an agent identity token");
            }
        }
    }
    issues
}

fn network_check() -> DoctorCheck {
    let mut details = Vec::new();
    push_proxy_env_details(&mut details);

    let mut status = CheckStatus::Ok;
    let mut summary = "network-related environment looks readable".to_string();
    for name in ["CODEX_CA_CERTIFICATE", "SSL_CERT_FILE"] {
        if let Some(raw) = env::var_os(name) {
            let path = PathBuf::from(raw);
            match std::fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() => {
                    if let Err(err) = read_probe_file(&path) {
                        status = CheckStatus::Warning;
                        summary = "custom CA env var points at an unreadable file".to_string();
                        details.push(format!("{name}: {} ({err})", path.display()));
                    } else {
                        details.push(format!("{name}: readable file {}", path.display()));
                    }
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

fn push_proxy_env_details(details: &mut Vec<String>) {
    let present_proxy_vars = PROXY_ENV_VARS
        .iter()
        .copied()
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
}

fn read_probe_file(path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = [0_u8; 1];
    let _ = file.read(&mut buffer)?;
    Ok(())
}

async fn mcp_check(config: &Config) -> DoctorCheck {
    mcp_check_from_servers(config.mcp_servers.get()).await
}

async fn mcp_check_from_servers(servers: &HashMap<String, McpServerConfig>) -> DoctorCheck {
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
    let mut unreachable_required_http = Vec::new();
    let mut unreachable_optional_http = Vec::new();

    for (name, server) in servers {
        let disabled_server = !server.enabled || server.disabled_reason.is_some();
        if disabled_server {
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
                if disabled_server {
                    continue;
                }
                if let Some(cwd) = cwd
                    && !cwd.exists()
                {
                    missing_env.push(format!("{name}: cwd does not exist ({})", cwd.display()));
                }
                if command.trim().is_empty() {
                    missing_env.push(format!("{name}: stdio command is empty"));
                } else if let Err(err) =
                    stdio_command_resolves(command, cwd.as_deref(), env.as_ref())
                {
                    missing_env.push(format!(
                        "{name}: stdio command {command:?} is not resolvable ({err})"
                    ));
                }
                if let Some(env) = env {
                    for key in env.keys().filter(|key| key.trim().is_empty()) {
                        missing_env.push(format!("{name}: empty env key {key}"));
                    }
                }
                for env_var in env_vars {
                    if env_var.is_remote_source() {
                        missing_env.push(format!(
                            "{name}: env_vars entry `{}` uses source `remote`, which requires remote MCP stdio",
                            env_var.name()
                        ));
                    } else if !env_var_present(env_var.name()) {
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
                if disabled_server {
                    continue;
                }
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
                if let Err(err) = mcp_http_probe_url(url).await {
                    let detail = format!("{name}: {url} ({err})");
                    if server.required {
                        unreachable_required_http.push(detail);
                    } else {
                        unreachable_optional_http.push(detail);
                    }
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
        unreachable_required_http
            .iter()
            .map(|detail| format!("required reachability failed: {detail}")),
    );
    details.extend(
        unreachable_optional_http
            .iter()
            .map(|detail| format!("optional reachability failed: {detail}")),
    );

    let required_missing = servers.iter().any(|(name, server)| {
        server.required
            && missing_env
                .iter()
                .any(|missing| missing.starts_with(&format!("{name}:")))
    });
    let status = if required_missing || !unreachable_required_http.is_empty() {
        CheckStatus::Fail
    } else if !missing_env.is_empty() || !unreachable_optional_http.is_empty() {
        CheckStatus::Warning
    } else {
        CheckStatus::Ok
    };
    let summary = match status {
        CheckStatus::Ok => "MCP configuration is locally consistent",
        CheckStatus::Warning => "MCP configuration has optional issues",
        CheckStatus::Fail => "MCP configuration has failing required inputs or reachability",
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

#[derive(Clone, Debug)]
struct TerminalCheckInputs {
    info: TerminalInfo,
    env: BTreeMap<String, String>,
    present_env: BTreeSet<String>,
    no_color_flag: bool,
    stdin_is_terminal: bool,
    stdout_is_terminal: bool,
    stderr_is_terminal: bool,
    stream_supports_color: bool,
    terminal_size: Result<(u16, u16), String>,
    tmux_details: Vec<String>,
}

impl TerminalCheckInputs {
    fn detect(no_color_flag: bool) -> Self {
        let names = terminal_env_names();
        let (env, present_env) = collect_env_snapshot(&names);
        let terminal_size = crossterm::terminal::size().map_err(|err| err.to_string());
        let info = terminal_info();
        let tmux_details = if matches!(info.multiplexer, Some(Multiplexer::Tmux { .. })) {
            tmux_diagnostic_details()
        } else {
            Vec::new()
        };
        Self {
            info,
            env,
            present_env,
            no_color_flag,
            stdin_is_terminal: std::io::stdin().is_terminal(),
            stdout_is_terminal: std::io::stdout().is_terminal(),
            stderr_is_terminal: std::io::stderr().is_terminal(),
            stream_supports_color: supports_color::on(Stream::Stdout).is_some(),
            terminal_size,
            tmux_details,
        }
    }

    fn env_value(&self, name: &str) -> Option<&str> {
        self.env.get(name).map(String::as_str)
    }

    fn env_present(&self, name: &str) -> bool {
        self.present_env.contains(name)
    }
}

fn terminal_check(no_color_flag: bool) -> DoctorCheck {
    terminal_check_from_inputs(TerminalCheckInputs::detect(no_color_flag))
}

fn terminal_check_from_inputs(inputs: TerminalCheckInputs) -> DoctorCheck {
    let info = &inputs.info;
    let name = info.name;
    let mut details = vec![format!("terminal: {}", terminal_name(info))];
    if let Some(term_program) = info.term_program.as_deref() {
        details.push(format!("TERM_PROGRAM: {term_program}"));
    }
    if let Some(version) = info.version.as_deref() {
        details.push(format!("terminal version: {version}"));
    }
    if let Some(term) = info.term.as_deref() {
        details.push(format!("TERM: {term}"));
    }
    if let Some(multiplexer) = info.multiplexer.as_ref() {
        details.push(format!("multiplexer: {}", multiplexer_name(multiplexer)));
    }
    details.push(format!("stdin is terminal: {}", inputs.stdin_is_terminal));
    details.push(format!("stdout is terminal: {}", inputs.stdout_is_terminal));
    details.push(format!("stderr is terminal: {}", inputs.stderr_is_terminal));
    match &inputs.terminal_size {
        Ok((columns, rows)) => details.push(format!("terminal size: {columns}x{rows}")),
        Err(err) => details.push(format!("terminal size: unavailable ({err})")),
    }
    push_terminal_env_values(&mut details, &inputs, TERMINAL_DIMENSION_ENV_VARS);
    details.push(format!("color output: {}", color_output_summary(&inputs)));
    push_terminal_env_values(&mut details, &inputs, COLOR_ENV_VARS);
    let terminfo_warning = push_terminfo_details(&mut details, &inputs);
    let locale = effective_locale(&inputs);
    if let Some(locale) = locale.as_ref() {
        details.push(format!("effective locale: {locale}"));
    }
    push_presence_env_values(&mut details, &inputs, REMOTE_TERMINAL_ENV_VARS);
    details.extend(inputs.tmux_details.iter().cloned());

    let locale_warning = locale.as_deref().is_some_and(is_non_utf8_locale);
    let mut issues = Vec::new();
    if matches!(name, TerminalName::Dumb) {
        issues.push(
            DoctorIssue::new(
                CheckStatus::Fail,
                "TERM=dumb - colors and cursor control are disabled",
            )
            .measured("TERM=dumb")
            .expected("TERM=xterm-256color or another real terminal type")
            .remedy("set TERM to a real value, for example xterm-256color")
            .field("TERM"),
        );
    }
    if locale_warning {
        let measured = locale.unwrap_or_else(|| "unknown".to_string());
        issues.push(
            DoctorIssue::new(
                CheckStatus::Warning,
                "locale is not UTF-8 - unicode glyphs may render incorrectly",
            )
            .measured(measured)
            .expected("UTF-8 locale, for example en_US.UTF-8")
            .remedy("export LANG=en_US.UTF-8 or another UTF-8 locale")
            .field("effective locale"),
        );
    }
    if terminfo_warning {
        issues.push(
            DoctorIssue::new(
                CheckStatus::Fail,
                "TERMINFO unreadable - terminal capabilities are unknown",
            )
            .expected("readable terminfo file or directory")
            .remedy("check that $TERMINFO points to a readable directory")
            .field("TERMINFO")
            .field("TERMINFO_DIRS entry"),
        );
    }
    issues.extend(terminal_size_issues(&inputs));

    let status = issues
        .iter()
        .map(|issue| issue.severity)
        .max()
        .unwrap_or(CheckStatus::Ok);
    let summary = issues
        .first()
        .map(|issue| issue.cause.as_str())
        .unwrap_or("terminal metadata was detected");
    let mut check = DoctorCheck::new("terminal.env", "terminal", status, summary).details(details);
    for issue in issues {
        check = check.issue(issue);
    }
    check
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
        Multiplexer::Zellij { version } => match version {
            Some(version) => format!("zellij {version}"),
            None => "zellij".to_string(),
        },
    }
}

fn terminal_env_names() -> BTreeSet<&'static str> {
    let mut names = BTreeSet::from(["TERM", "TERM_PROGRAM", "TERM_PROGRAM_VERSION"]);
    names.extend(COLOR_ENV_VARS.iter().copied());
    names.extend(TERMINAL_DIMENSION_ENV_VARS.iter().copied());
    names.extend(TERMINFO_ENV_VARS.iter().copied());
    names.extend(LOCALE_ENV_VARS.iter().copied());
    names.extend(REMOTE_TERMINAL_ENV_VARS.iter().copied());
    names
}

fn collect_env_snapshot(
    names: &BTreeSet<&'static str>,
) -> (BTreeMap<String, String>, BTreeSet<String>) {
    let mut values = BTreeMap::new();
    let mut present = BTreeSet::new();
    for name in names {
        if let Some(raw) = env::var_os(name) {
            present.insert((*name).to_string());
            let value = raw.to_string_lossy().trim().to_string();
            if !value.is_empty() {
                values.insert((*name).to_string(), value);
            }
        }
    }
    (values, present)
}

fn push_terminal_env_values(
    details: &mut Vec<String>,
    inputs: &TerminalCheckInputs,
    names: &[&str],
) {
    for name in names {
        if let Some(value) = inputs.env_value(name) {
            details.push(format!("{name}: {value}"));
        } else if inputs.env_present(name) {
            details.push(format!("{name}: present"));
        }
    }
}

fn push_presence_env_values(
    details: &mut Vec<String>,
    inputs: &TerminalCheckInputs,
    names: &[&str],
) {
    for name in names {
        if inputs.env_present(name) {
            details.push(format!("{name}: present"));
        }
    }
}

fn color_output_summary(inputs: &TerminalCheckInputs) -> String {
    if should_enable_color(
        inputs.no_color_flag,
        inputs.env_present("NO_COLOR"),
        inputs.env_value("TERM"),
        inputs.stdout_is_terminal,
        inputs.stream_supports_color,
    ) {
        return "enabled".to_string();
    }

    let reason = if inputs.no_color_flag {
        "--no-color"
    } else if inputs.env_present("NO_COLOR") {
        "NO_COLOR"
    } else if inputs.env_value("TERM") == Some("dumb") {
        "TERM=dumb"
    } else if !inputs.stdout_is_terminal {
        "stdout is not a terminal"
    } else if !inputs.stream_supports_color {
        "terminal color support not detected"
    } else {
        "disabled"
    };
    format!("disabled ({reason})")
}

fn push_terminfo_details(details: &mut Vec<String>, inputs: &TerminalCheckInputs) -> bool {
    let mut has_warning = false;
    if let Some(raw) = inputs.env_value("TERMINFO") {
        let path = PathBuf::from(raw);
        let (status, warning) = terminal_path_readiness(&path);
        details.push(format!("TERMINFO: {} ({status})", path.display()));
        has_warning |= warning;
    }
    if let Some(raw) = inputs.env_value("TERMINFO_DIRS") {
        for path in env::split_paths(raw).filter(|path| !path.as_os_str().is_empty()) {
            let (status, warning) = terminal_path_readiness(&path);
            details.push(format!(
                "TERMINFO_DIRS entry: {} ({status})",
                path.display()
            ));
            has_warning |= warning;
        }
    } else if inputs.env_present("TERMINFO_DIRS") {
        details.push("TERMINFO_DIRS: present".to_string());
    }
    has_warning
}

fn terminal_path_readiness(path: &Path) -> (String, bool) {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => match std::fs::read_dir(path) {
            Ok(_) => ("dir".to_string(), false),
            Err(err) => (format!("dir unreadable: {err}"), true),
        },
        Ok(metadata) if metadata.is_file() => match read_probe_file(path) {
            Ok(_) => ("file".to_string(), false),
            Err(err) => (format!("file unreadable: {err}"), true),
        },
        Ok(_) => ("not a file or directory".to_string(), true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => ("missing".to_string(), true),
        Err(err) => (err.to_string(), true),
    }
}

fn effective_locale(inputs: &TerminalCheckInputs) -> Option<String> {
    LOCALE_ENV_VARS
        .iter()
        .find_map(|name| inputs.env_value(name).map(ToString::to_string))
}

fn is_non_utf8_locale(locale: &str) -> bool {
    let locale = locale.to_ascii_lowercase();
    !(locale.contains("utf-8") || locale.contains("utf8"))
}

fn terminal_size_issues(inputs: &TerminalCheckInputs) -> Vec<DoctorIssue> {
    let mut issues = Vec::new();
    if let Ok((columns, rows)) = inputs.terminal_size {
        if columns > 0 && columns < NARROW_TERMINAL_COLUMNS {
            issues.push(
                DoctorIssue::new(
                    CheckStatus::Warning,
                    format!("width {columns} cols - output may wrap (recommended >=80)"),
                )
                .measured(format!("{columns} x {rows}"))
                .expected(format!(">= {NARROW_TERMINAL_COLUMNS} columns"))
                .remedy("resize the window to at least 80 columns")
                .field("terminal size"),
            );
        }
        if rows > 0 && rows < NARROW_TERMINAL_ROWS {
            issues.push(
                DoctorIssue::new(
                    CheckStatus::Warning,
                    format!("height {rows} rows - content may scroll off (recommended >=24)"),
                )
                .measured(format!("{columns} x {rows}"))
                .expected(format!(">= {NARROW_TERMINAL_ROWS} rows"))
                .remedy("resize the window to at least 24 rows")
                .field("terminal size"),
            );
        }
    }

    if let Some(columns) = inputs
        .env_value("COLUMNS")
        .and_then(|columns| columns.parse::<u16>().ok())
        && columns > 0
        && columns < NARROW_TERMINAL_COLUMNS
    {
        issues.push(
            DoctorIssue::new(
                CheckStatus::Warning,
                format!("COLUMNS={columns} - output may wrap (recommended >=80)"),
            )
            .measured(format!("{columns} columns"))
            .expected(format!(">= {NARROW_TERMINAL_COLUMNS} columns"))
            .remedy("resize the window to at least 80 columns")
            .field("COLUMNS"),
        );
    }
    if let Some(rows) = inputs
        .env_value("LINES")
        .and_then(|rows| rows.parse::<u16>().ok())
        && rows > 0
        && rows < NARROW_TERMINAL_ROWS
    {
        issues.push(
            DoctorIssue::new(
                CheckStatus::Warning,
                format!("LINES={rows} - content may scroll off (recommended >=24)"),
            )
            .measured(format!("{rows} rows"))
            .expected(format!(">= {NARROW_TERMINAL_ROWS} rows"))
            .remedy("resize the window to at least 24 rows")
            .field("LINES"),
        );
    }

    issues
}

fn tmux_diagnostic_details() -> Vec<String> {
    let mut details = Vec::new();
    push_tmux_display_detail(&mut details, "tmux client termtype", "#{client_termtype}");
    push_tmux_display_detail(&mut details, "tmux client termname", "#{client_termname}");
    for option in TMUX_OPTION_NAMES {
        let value = tmux_option_value(option).unwrap_or_else(|| "unavailable".to_string());
        details.push(format!("tmux {option}: {value}"));
    }
    details
}

fn push_tmux_display_detail(details: &mut Vec<String>, label: &str, format: &str) {
    if let Some(value) = tmux_display_message(format) {
        details.push(format!("{label}: {value}"));
    }
}

fn tmux_option_value(option: &str) -> Option<String> {
    let output = Command::new("tmux")
        .args(["show-options", "-gqv", option])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    non_empty_trimmed(String::from_utf8(output.stdout).ok()?)
}

fn tmux_display_message(format: &str) -> Option<String> {
    let output = Command::new("tmux")
        .args(["display-message", "-p", format])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    non_empty_trimmed(String::from_utf8(output.stdout).ok()?)
}

fn non_empty_trimmed(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

async fn state_check(config: &Config) -> DoctorCheck {
    let mut details = Vec::new();
    path_readiness(&mut details, "CODEX_HOME", &config.codex_home);
    path_readiness(&mut details, "log dir", &config.log_dir);
    path_readiness(&mut details, "sqlite home", &config.sqlite_home);
    let state_db = codex_state::state_db_path(&config.sqlite_home);
    let log_db = codex_state::logs_db_path(&config.sqlite_home);
    path_readiness(&mut details, "state DB", &state_db);
    path_readiness(&mut details, "log DB", &log_db);
    let mut integrity_failures = Vec::new();
    sqlite_integrity_detail(&mut details, &mut integrity_failures, "state DB", &state_db).await;
    sqlite_integrity_detail(&mut details, &mut integrity_failures, "log DB", &log_db).await;
    rollout_stats_details(&mut details, &config.codex_home);
    standalone_release_cache_details(&mut details);

    let status = if integrity_failures.is_empty() {
        CheckStatus::Ok
    } else {
        CheckStatus::Fail
    };
    let summary = if status == CheckStatus::Ok {
        "state paths and databases are inspectable"
    } else {
        "state database integrity check failed"
    };
    let mut check = DoctorCheck::new("state.paths", "state", status, summary).details(details);
    if status == CheckStatus::Fail {
        check = check
            .remediation("Back up CODEX_HOME, then remove or repair the affected SQLite database.");
    }
    check
}

async fn sqlite_integrity_detail(
    details: &mut Vec<String>,
    integrity_failures: &mut Vec<String>,
    label: &str,
    path: &Path,
) {
    if !path.is_file() {
        details.push(format!("{label} integrity: skipped (missing)"));
        return;
    }

    match codex_state::sqlite_integrity_check(path).await {
        Ok(rows) if rows.iter().all(|row| row == "ok") => {
            details.push(format!("{label} integrity: ok"));
        }
        Ok(rows) => {
            let message = format!("{label} integrity: {}", rows.join("; "));
            integrity_failures.push(message.clone());
            details.push(message);
        }
        Err(err) => {
            let message = format!("{label} integrity: {err}");
            integrity_failures.push(message.clone());
            details.push(message);
        }
    }
}

fn rollout_stats_details(details: &mut Vec<String>, codex_home: &Path) {
    let active = collect_rollout_stats(&codex_home.join("sessions"));
    let archived = collect_rollout_stats(&codex_home.join("archived_sessions"));
    push_rollout_stats_detail(details, "active rollout files", active);
    push_rollout_stats_detail(details, "archived rollout files", archived);
}

fn push_rollout_stats_detail(details: &mut Vec<String>, label: &str, stats: RolloutStats) {
    match stats.error {
        Some(error) => details.push(format!("{label}: scan failed ({error})")),
        None => details.push(format!(
            "{label}: {} files, {} total bytes, {} average bytes",
            stats.files,
            stats.total_bytes,
            stats.average_bytes()
        )),
    }
}

#[derive(Default)]
struct RolloutStats {
    files: u64,
    total_bytes: u64,
    error: Option<String>,
}

impl RolloutStats {
    fn average_bytes(&self) -> u64 {
        if self.files == 0 {
            0
        } else {
            self.total_bytes / self.files
        }
    }
}

fn collect_rollout_stats(root: &Path) -> RolloutStats {
    let mut stats = RolloutStats::default();
    collect_rollout_stats_inner(root, &mut stats);
    stats
}

fn collect_rollout_stats_inner(path: &Path, stats: &mut RolloutStats) {
    if stats.error.is_some() {
        return;
    }
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            stats.error = Some(err.to_string());
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                stats.error = Some(err.to_string());
                return;
            }
        };
        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(err) => {
                stats.error = Some(err.to_string());
                return;
            }
        };
        if metadata.is_dir() {
            collect_rollout_stats_inner(&path, stats);
        } else if metadata.is_file() && is_rollout_file(&path) {
            stats.files += 1;
            stats.total_bytes = stats.total_bytes.saturating_add(metadata.len());
        }
    }
}

fn is_rollout_file(path: &Path) -> bool {
    path.extension() == Some(OsStr::new("jsonl"))
        && path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("rollout-"))
}

async fn websocket_reachability_check(
    config: &Config,
    auth_manager: Option<Arc<AuthManager>>,
) -> DoctorCheck {
    let provider = &config.model_provider;
    let mut details = vec![
        format!("model provider: {}", config.model_provider_id),
        format!("provider name: {}", provider.name),
        format!("wire API: {}", provider.wire_api),
        format!("supports websockets: {}", provider.supports_websockets),
    ];
    push_proxy_env_details(&mut details);

    if !provider.supports_websockets {
        return DoctorCheck::new(
            "network.websocket_reachability",
            "websocket",
            CheckStatus::Ok,
            "Responses WebSocket is not enabled for the active provider",
        )
        .details(details);
    }

    details.push(format!(
        "connect timeout: {} ms",
        provider.websocket_connect_timeout().as_millis()
    ));

    let runtime_provider = create_model_provider(provider.clone(), auth_manager);
    let auth = runtime_provider.auth().await;
    details.push(format!(
        "auth mode: {}",
        auth.as_ref().map(auth_mode_name).unwrap_or("none")
    ));

    let api_provider = match runtime_provider.api_provider().await {
        Ok(api_provider) => api_provider,
        Err(err) => {
            return websocket_probe_warning(
                "Responses WebSocket provider setup failed",
                details,
                format!("provider setup failed: {err}"),
            );
        }
    };
    match api_provider.websocket_url_for_path("responses") {
        Ok(url) => {
            details.push(format!("endpoint: {url}"));
            if let Some(host) = url.host_str()
                && let Some(port) = url.port_or_known_default()
            {
                details.extend(dns_address_family_details(host, port).await);
            }
        }
        Err(err) => {
            return websocket_probe_warning(
                "Responses WebSocket endpoint could not be built",
                details,
                format!("endpoint build failed: {err}"),
            );
        }
    }

    let api_auth = match runtime_provider.api_auth().await {
        Ok(api_auth) => api_auth,
        Err(err) => {
            return websocket_probe_warning(
                "Responses WebSocket auth could not be resolved",
                details,
                format!("auth resolution failed: {err}"),
            );
        }
    };

    let mut extra_headers = HeaderMap::new();
    extra_headers.insert(
        OPENAI_BETA_HEADER,
        HeaderValue::from_static(RESPONSES_WEBSOCKETS_V2_BETA_HEADER_VALUE),
    );
    let client = ResponsesWebsocketClient::new(api_provider, api_auth);
    match tokio::time::timeout(
        provider.websocket_connect_timeout(),
        client.probe_handshake(
            extra_headers,
            default_headers(),
            WEBSOCKET_IMMEDIATE_CLOSE_GRACE,
        ),
    )
    .await
    {
        Ok(Ok(probe)) => {
            details.push(format!("handshake result: HTTP {}", probe.status));
            details.push(format!("reasoning header: {}", probe.reasoning_included));
            details.push(format!(
                "models etag present: {}",
                probe.models_etag_present
            ));
            details.push(format!(
                "server model present: {}",
                probe.server_model_present
            ));
            if let Some(close) = probe.immediate_close {
                details.push(format!("immediate close code: {}", close.code));
                details.push(format!("immediate close reason: {}", close.reason));
                return DoctorCheck::new(
                    "network.websocket_reachability",
                    "websocket",
                    CheckStatus::Warning,
                    "Responses WebSocket closed immediately after handshake",
                )
                .details(details)
                .remediation(
                    "Check proxy, VPN, firewall, DNS, custom CA, and WebSocket policy support.",
                );
            }
            DoctorCheck::new(
                "network.websocket_reachability",
                "websocket",
                CheckStatus::Ok,
                "Responses WebSocket handshake succeeded",
            )
            .details(details)
        }
        Ok(Err(err)) => websocket_probe_warning(
            "Responses WebSocket failed; HTTPS fallback may still work",
            details,
            websocket_error_detail(&err),
        ),
        Err(_) => websocket_probe_warning(
            "Responses WebSocket timed out; HTTPS fallback may still work",
            details,
            "handshake timed out".to_string(),
        ),
    }
}

fn websocket_probe_warning(
    summary: &'static str,
    mut details: Vec<String>,
    error_detail: String,
) -> DoctorCheck {
    details.push(error_detail);
    DoctorCheck::new(
        "network.websocket_reachability",
        "websocket",
        CheckStatus::Warning,
        summary,
    )
    .details(details)
    .remediation("Check proxy, VPN, firewall, DNS, custom CA, and WebSocket policy support.")
}

fn websocket_error_detail(err: &ApiError) -> String {
    match err {
        ApiError::Transport(transport) => format!("handshake transport error: {transport}"),
        ApiError::Api { status, message } => {
            format!("handshake API error: {status} {message}")
        }
        ApiError::Stream(message) => format!("handshake stream error: {message}"),
        ApiError::ContextWindowExceeded
        | ApiError::QuotaExceeded
        | ApiError::UsageNotIncluded
        | ApiError::Retryable { .. }
        | ApiError::RateLimit(_)
        | ApiError::InvalidRequest { .. }
        | ApiError::CyberPolicy { .. }
        | ApiError::ServerOverloaded => format!("handshake error: {err}"),
    }
}

fn auth_mode_name(auth: &CodexAuth) -> &'static str {
    match auth.auth_mode() {
        codex_app_server_protocol::AuthMode::ApiKey => "api_key",
        codex_app_server_protocol::AuthMode::Chatgpt => "chatgpt",
        codex_app_server_protocol::AuthMode::ChatgptAuthTokens => "chatgpt_auth_tokens",
        codex_app_server_protocol::AuthMode::AgentIdentity => "agent_identity",
    }
}

async fn dns_address_family_details(host: &str, port: u16) -> Vec<String> {
    match tokio::net::lookup_host((host, port)).await {
        Ok(addresses) => {
            let addresses = addresses.collect::<Vec<_>>();
            let ipv4_count = addresses
                .iter()
                .filter(|address| matches!(address.ip(), IpAddr::V4(_)))
                .count();
            let ipv6_count = addresses
                .iter()
                .filter(|address| matches!(address.ip(), IpAddr::V6(_)))
                .count();
            let first_family = addresses
                .first()
                .map(|address| match address.ip() {
                    IpAddr::V4(_) => "IPv4",
                    IpAddr::V6(_) => "IPv6",
                })
                .unwrap_or("none");
            vec![format!(
                "DNS: {ipv4_count} IPv4, {ipv6_count} IPv6, first {first_family}"
            )]
        }
        Err(err) => vec![format!("DNS: lookup failed ({err})")],
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReachabilityPlan {
    description: String,
    endpoints: Vec<ReachabilityEndpoint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReachabilityEndpoint {
    label: String,
    url: String,
    required: bool,
    route_probe_url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderAuthReachabilityMode {
    NotRequired,
    ApiKey,
    Chatgpt,
}

impl ProviderAuthReachabilityMode {
    fn description(self) -> &'static str {
        match self {
            Self::NotRequired => "provider auth",
            Self::ApiKey => "API key auth",
            Self::Chatgpt => "ChatGPT auth",
        }
    }
}

fn provider_reachability_plan(config: &Config) -> ReachabilityPlan {
    let stored_auth =
        load_auth_dot_json(&config.codex_home, config.cli_auth_credentials_store_mode)
            .ok()
            .flatten();
    let mode = provider_auth_reachability_mode_from_auth(
        config.model_provider.requires_openai_auth,
        env_var_present,
        stored_auth.as_ref(),
    );
    provider_reachability_plan_from_parts(
        mode,
        &config.model_provider_id,
        &config.model_provider.name,
        config.model_provider.base_url.as_deref(),
        config.model_provider.query_params.as_ref(),
        config.model_provider.is_amazon_bedrock(),
        &config.chatgpt_base_url,
    )
}

fn default_reachability_plan() -> ReachabilityPlan {
    provider_reachability_plan_from_parts(
        ProviderAuthReachabilityMode::Chatgpt,
        "openai",
        "OpenAI",
        /*provider_base_url*/ None,
        /*provider_query_params*/ None,
        /*is_amazon_bedrock*/ false,
        "https://chatgpt.com/backend-api/",
    )
}

fn provider_auth_reachability_mode_from_auth(
    requires_openai_auth: bool,
    env_var_present: impl Fn(&str) -> bool,
    stored_auth: Option<&AuthDotJson>,
) -> ProviderAuthReachabilityMode {
    if !requires_openai_auth {
        return ProviderAuthReachabilityMode::NotRequired;
    }
    if env_var_present(OPENAI_API_KEY_ENV_VAR) || env_var_present(CODEX_API_KEY_ENV_VAR) {
        return ProviderAuthReachabilityMode::ApiKey;
    }
    if env_var_present(CODEX_ACCESS_TOKEN_ENV_VAR) {
        return ProviderAuthReachabilityMode::Chatgpt;
    }
    match stored_auth.map(stored_auth_mode_value) {
        Some(codex_app_server_protocol::AuthMode::ApiKey) => ProviderAuthReachabilityMode::ApiKey,
        Some(
            codex_app_server_protocol::AuthMode::Chatgpt
            | codex_app_server_protocol::AuthMode::ChatgptAuthTokens
            | codex_app_server_protocol::AuthMode::AgentIdentity,
        )
        | None => ProviderAuthReachabilityMode::Chatgpt,
    }
}

fn provider_reachability_plan_from_parts(
    mode: ProviderAuthReachabilityMode,
    provider_id: &str,
    provider_name: &str,
    provider_base_url: Option<&str>,
    provider_query_params: Option<&HashMap<String, String>>,
    is_amazon_bedrock: bool,
    chatgpt_base_url: &str,
) -> ReachabilityPlan {
    let provider_route_probe_url = provider_base_url
        .or_else(|| {
            (mode == ProviderAuthReachabilityMode::ApiKey).then_some("https://api.openai.com/v1")
        })
        .and_then(|url| {
            should_probe_models_route(provider_name, url, is_amazon_bedrock)
                .then(|| provider_url_for_path(url, "models", provider_query_params))
        });
    let endpoints = match mode {
        ProviderAuthReachabilityMode::ApiKey => vec![ReachabilityEndpoint {
            label: format!("{provider_id} API"),
            url: provider_base_url
                .unwrap_or("https://api.openai.com/v1")
                .to_string(),
            required: true,
            route_probe_url: provider_route_probe_url,
        }],
        ProviderAuthReachabilityMode::Chatgpt => vec![ReachabilityEndpoint {
            label: "ChatGPT".to_string(),
            url: chatgpt_base_url.to_string(),
            required: true,
            route_probe_url: None,
        }],
        ProviderAuthReachabilityMode::NotRequired => provider_base_url
            .map(|url| {
                vec![ReachabilityEndpoint {
                    label: format!("{provider_id} API"),
                    url: url.to_string(),
                    required: true,
                    route_probe_url: provider_route_probe_url,
                }]
            })
            .unwrap_or_default(),
    };
    ReachabilityPlan {
        description: mode.description().to_string(),
        endpoints,
    }
}

fn should_probe_models_route(provider_name: &str, base_url: &str, is_amazon_bedrock: bool) -> bool {
    !is_amazon_bedrock && !is_azure_responses_provider(provider_name, Some(base_url))
}

fn provider_url_for_path(
    base_url: &str,
    path: &str,
    query_params: Option<&HashMap<String, String>>,
) -> String {
    let base = base_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let mut url = if path.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{path}")
    };

    if let Some(params) = query_params
        && !params.is_empty()
    {
        let separator = if url.contains('?') { '&' } else { '?' };
        url.push(separator);
        url.push_str(
            &params
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&"),
        );
    }

    url
}

async fn provider_reachability_check(plan: ReachabilityPlan) -> DoctorCheck {
    let mut details = vec![format!("reachability mode: {}", plan.description)];
    if plan.endpoints.is_empty() {
        details.push("active provider endpoint: none configured".to_string());
        return DoctorCheck::new(
            "network.provider_reachability",
            "reachability",
            CheckStatus::Ok,
            "active provider has no HTTP endpoint to probe",
        )
        .details(details);
    }

    let mut failures = Vec::new();
    let mut optional_failures = Vec::new();
    let mut route_failures = Vec::new();
    let mut route_warnings = Vec::new();
    let mut issues = Vec::new();
    for endpoint in plan.endpoints {
        match http_probe_url(&endpoint.url).await {
            Ok(status) => details.push(format!(
                "{} base URL: {} reachable ({status})",
                endpoint.label, endpoint.url
            )),
            Err(err) => {
                let requirement = if endpoint.required {
                    "required"
                } else {
                    "optional"
                };
                details.push(format!(
                    "{} base URL: {} {err} ({requirement})",
                    endpoint.label, endpoint.url
                ));
                if endpoint.required {
                    failures.push(endpoint.url);
                } else {
                    optional_failures.push(endpoint.url);
                }
                continue;
            }
        }

        let Some(route_probe_url) = endpoint.route_probe_url.as_deref() else {
            continue;
        };
        match provider_route_probe_url(route_probe_url).await {
            RouteProbeOutcome::Ok(status) => {
                details.push(format!(
                    "{} route probe: {route_probe_url} route exists ({status})",
                    endpoint.label,
                ));
            }
            RouteProbeOutcome::Warning(status) => {
                details.push(format!(
                    "{} route probe: {route_probe_url} returned {status} (warning)",
                    endpoint.label,
                ));
                route_warnings.push(route_probe_url.to_string());
            }
            RouteProbeOutcome::Fail(status) => {
                details.push(format!(
                    "{} route probe: {route_probe_url} returned {status} (required)",
                    endpoint.label,
                ));
                route_failures.push(route_probe_url.to_string());
                issues.push(
                    DoctorIssue::new(
                        CheckStatus::Fail,
                        "provider base URL route returned 404 - verify the configured API prefix",
                    )
                    .measured(format!("{route_probe_url} returned {status}"))
                    .expected("GET /models returns 2xx, 401, or 403")
                    .remedy("Set base_url to the provider API root, for example https://api.openai.com/v1")
                    .field("route probe"),
                );
            }
            RouteProbeOutcome::TransportError(err) => {
                details.push(format!(
                    "{} route probe: {route_probe_url} {err} (required)",
                    endpoint.label,
                ));
                route_failures.push(route_probe_url.to_string());
                issues.push(
                    DoctorIssue::new(
                        CheckStatus::Fail,
                        "provider route probe could not connect - verify network access to the provider API",
                    )
                    .measured(format!("{route_probe_url} {err}"))
                    .expected("GET /models completes")
                    .remedy("Check proxy, VPN, firewall, DNS, and custom CA configuration.")
                    .field("route probe"),
                );
            }
        }
    }

    let (status, summary) = provider_reachability_outcome(
        failures.len() + route_failures.len(),
        optional_failures.len() + route_warnings.len(),
    );
    let mut check = DoctorCheck::new(
        "network.provider_reachability",
        "reachability",
        status,
        summary,
    )
    .details(details);
    for issue in issues {
        check = check.issue(issue);
    }
    if status != CheckStatus::Ok {
        check = check.remediation("Check proxy, VPN, firewall, DNS, and custom CA configuration.");
    }
    check
}

enum RouteProbeOutcome {
    Ok(String),
    Warning(String),
    Fail(String),
    TransportError(String),
}

async fn provider_route_probe_url(url: &str) -> RouteProbeOutcome {
    match http_get_probe_status_with_timeout(url, Duration::from_secs(3)).await {
        Ok(status) if (200..300).contains(&status) || matches!(status, 401 | 403) => {
            RouteProbeOutcome::Ok(format!("HTTP {status}"))
        }
        Ok(404) => RouteProbeOutcome::Fail("HTTP 404".to_string()),
        Ok(status) => RouteProbeOutcome::Warning(format!("HTTP {status}")),
        Err(err) => RouteProbeOutcome::TransportError(err),
    }
}

fn provider_reachability_outcome(
    required_failures: usize,
    warnings: usize,
) -> (CheckStatus, &'static str) {
    match (required_failures, warnings) {
        (0, 0) => (
            CheckStatus::Ok,
            "active provider endpoints are reachable over HTTP",
        ),
        (0, _) => (
            CheckStatus::Warning,
            "provider endpoint checks returned warnings",
        ),
        (_, _) => (
            CheckStatus::Fail,
            "one or more required provider endpoints are unreachable over HTTP",
        ),
    }
}

async fn http_probe_url(url: &str) -> Result<String, String> {
    http_probe_url_with_timeout(url, Duration::from_secs(3)).await
}

async fn mcp_http_probe_url(url: &str) -> Result<String, String> {
    mcp_http_probe_url_with_timeout(url, Duration::from_secs(3)).await
}

async fn mcp_http_probe_url_with_timeout(url: &str, timeout: Duration) -> Result<String, String> {
    match http_probe_url_with_timeout(url, timeout).await {
        Ok(status) => Ok(status),
        Err(head_err) => match http_get_probe_url_with_timeout(url, timeout).await {
            Ok(status) => Ok(status),
            Err(get_err) => Err(format!("HEAD {head_err}; GET {get_err}")),
        },
    }
}

async fn http_probe_url_with_timeout(url: &str, timeout: Duration) -> Result<String, String> {
    let response = build_reqwest_client()
        .head(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|err| {
            if err.is_timeout() {
                "request timed out".to_string()
            } else if err.is_connect() {
                "connect failed".to_string()
            } else if err.is_builder() {
                "request could not be built".to_string()
            } else {
                err.to_string()
            }
        })?;
    Ok(format!("HTTP {}", response.status().as_u16()))
}

async fn http_get_probe_url_with_timeout(url: &str, timeout: Duration) -> Result<String, String> {
    http_get_probe_status_with_timeout(url, timeout)
        .await
        .map(|status| format!("HTTP {status}"))
}

async fn http_get_probe_status_with_timeout(url: &str, timeout: Duration) -> Result<u16, String> {
    let response = build_reqwest_client()
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|err| {
            if err.is_timeout() {
                "request timed out".to_string()
            } else if err.is_connect() {
                "connect failed".to_string()
            } else if err.is_builder() {
                "request could not be built".to_string()
            } else {
                err.to_string()
            }
        })?;
    Ok(response.status().as_u16())
}

fn stdio_command_resolves(
    command: &str,
    cwd: Option<&Path>,
    server_env: Option<&HashMap<String, String>>,
) -> Result<(), String> {
    let command_path = Path::new(command);
    if command_path.is_absolute() {
        return executable_path_exists(command_path);
    }

    if command_path.components().count() > 1 {
        let base = cwd
            .map(Path::to_path_buf)
            .or_else(|| env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        return executable_path_exists(&base.join(command_path));
    }

    let Some(path_env) = server_env
        .and_then(|env| env.get("PATH").map(String::as_str))
        .map(std::ffi::OsString::from)
        .or_else(|| env::var_os("PATH"))
    else {
        return Err("PATH is not set".to_string());
    };

    for dir in env::split_paths(&path_env) {
        let candidate = dir.join(command);
        if executable_path_exists(&candidate).is_ok() {
            return Ok(());
        }
        #[cfg(windows)]
        {
            let pathext = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
            for extension in pathext.split(';').filter(|extension| !extension.is_empty()) {
                let candidate = dir.join(format!("{command}{extension}"));
                if executable_path_exists(&candidate).is_ok() {
                    return Ok(());
                }
            }
        }
    }
    Err("not found on PATH".to_string())
}

fn executable_path_exists(path: &Path) -> Result<(), String> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => executable_file_permission(path, &metadata),
        Ok(_) => Err("path is not a file".to_string()),
        Err(err) => Err(err.to_string()),
    }
}

#[cfg(unix)]
fn executable_file_permission(path: &Path, metadata: &std::fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o111 == 0 {
        Err(format!("{} is not executable", path.display()))
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
fn executable_file_permission(_path: &Path, _metadata: &std::fs::Metadata) -> Result<(), String> {
    Ok(())
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
        show_details: !command.summary,
        show_all: command.all,
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
    use std::io::Read;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::Mutex;

    use clap::Parser;
    use codex_protocol::config_types::SandboxMode;
    use pretty_assertions::assert_eq;

    use super::*;

    #[derive(Default)]
    struct RecordingProgress {
        events: Mutex<Vec<String>>,
    }

    impl RecordingProgress {
        fn events(&self) -> Vec<String> {
            self.events.lock().expect("events lock").clone()
        }
    }

    impl DoctorProgress for RecordingProgress {
        fn begin(&self, label: &'static str) {
            self.events
                .lock()
                .expect("events lock")
                .push(format!("begin {label}"));
        }

        fn heartbeat(&self, label: &'static str, elapsed: Duration) {
            self.events
                .lock()
                .expect("events lock")
                .push(format!("heartbeat {label} {}", elapsed.as_secs()));
        }

        fn finish(&self, label: &'static str, status: CheckStatus) {
            self.events
                .lock()
                .expect("events lock")
                .push(format!("finish {label} {status:?}"));
        }

        fn settle(&self) {
            self.events
                .lock()
                .expect("events lock")
                .push("settle".to_string());
        }
    }

    fn respond_once(listener: &TcpListener, response: &[u8]) {
        let (mut stream, _) = listener.accept().expect("accept probe request");
        let mut request = [0; 1024];
        let _ = stream.read(&mut request);
        stream.write_all(response).expect("write response");
    }

    #[test]
    fn overall_status_prefers_fail() {
        let checks = vec![
            DoctorCheck::new("a", "config", CheckStatus::Warning, "warning"),
            DoctorCheck::new("b", "auth", CheckStatus::Fail, "fail"),
        ];
        assert_eq!(overall_status(&checks), CheckStatus::Fail);
    }

    #[test]
    fn run_sync_check_notifies_progress() {
        let progress_impl = Arc::new(RecordingProgress::default());
        let progress: Arc<dyn DoctorProgress> = progress_impl.clone();

        let check = run_sync_check("test", progress, || {
            DoctorCheck::new("test", "test", CheckStatus::Ok, "ok")
        });

        assert_eq!(check.status, CheckStatus::Ok);
        assert_eq!(
            progress_impl.events(),
            vec!["begin test".to_string(), "finish test Ok".to_string()]
        );
    }

    #[tokio::test]
    async fn run_async_check_notifies_progress() {
        let progress_impl = Arc::new(RecordingProgress::default());
        let progress: Arc<dyn DoctorProgress> = progress_impl.clone();

        let check = run_async_check("test", progress, async {
            DoctorCheck::new("test", "test", CheckStatus::Warning, "warning")
        })
        .await;

        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(
            progress_impl.events(),
            vec!["begin test".to_string(), "finish test Warning".to_string()]
        );
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
    fn config_overrides_from_interactive_preserves_global_options() {
        let interactive = TuiCli::parse_from([
            "codex",
            "--oss",
            "--local-provider",
            "ollama",
            "--model",
            "llama3.2",
            "--cd",
            "/tmp",
            "--sandbox",
            "danger-full-access",
            "--ask-for-approval",
            "never",
            "--add-dir",
            "/var/tmp",
        ]);
        let arg0_paths = Arg0DispatchPaths {
            codex_self_exe: Some(PathBuf::from("/bin/codex")),
            codex_linux_sandbox_exe: Some(PathBuf::from("/bin/codex-linux-sandbox")),
            main_execve_wrapper_exe: Some(PathBuf::from("/bin/codex-execve-wrapper")),
        };

        let overrides = config_overrides_from_interactive(&interactive, &arg0_paths);

        assert_eq!(overrides.model.as_deref(), Some("llama3.2"));
        assert_eq!(overrides.model_provider.as_deref(), Some("ollama"));
        assert_eq!(overrides.cwd.as_deref(), Some(Path::new("/tmp")));
        assert_eq!(overrides.approval_policy, Some(AskForApproval::Never));
        assert_eq!(overrides.sandbox_mode, Some(SandboxMode::DangerFullAccess));
        assert_eq!(overrides.show_raw_agent_reasoning, Some(true));
        assert_eq!(
            overrides.additional_writable_roots,
            vec![PathBuf::from("/var/tmp")]
        );
        assert_eq!(overrides.codex_self_exe, arg0_paths.codex_self_exe);
        assert_eq!(
            overrides.codex_linux_sandbox_exe,
            arg0_paths.codex_linux_sandbox_exe
        );
        assert_eq!(
            overrides.main_execve_wrapper_exe,
            arg0_paths.main_execve_wrapper_exe
        );
    }

    #[test]
    fn redacted_json_report_structures_and_sanitizes_details() {
        let report = DoctorReport {
            schema_version: 1,
            generated_at: "0s since unix epoch".to_string(),
            overall_status: CheckStatus::Warning,
            codex_version: "0.0.0".to_string(),
            checks: vec![
                DoctorCheck::new(
                    "mcp.config",
                    "mcp",
                    CheckStatus::Warning,
                    "MCP configuration has optional issues",
                )
                .detail(
                    "optional reachability failed: remote: https://user:pass@example.com/mcp?x=abc (connect failed)",
                )
                .detail("OPENAI_API_KEY: sk-live-secret")
                .detail("duplicate: one")
                .detail("duplicate: two")
                .detail("freeform note")
                .issue(
                    DoctorIssue::new(
                        CheckStatus::Warning,
                        "remote https://user:pass@example.com/mcp?x=abc is unreachable",
                    )
                    .measured("https://user:pass@example.com/mcp?x=abc")
                    .expected("reachable MCP endpoint")
                    .remedy("Check https://user:pass@example.com/help?x=abc.")
                    .field("optional reachability failed"),
                )
                .remediation("Open https://user:pass@example.com/help?x=abc."),
            ],
        };

        let redacted_report = redacted_json_report(&report);
        let redacted = serde_json::to_string(&redacted_report).expect("serialize report");
        let json = serde_json::to_value(redacted_report).expect("report should serialize");

        assert!(!redacted.contains("user:pass"));
        assert!(!redacted.contains("x=abc"));
        assert!(!redacted.contains("sk-live-secret"));
        assert!(redacted.contains("https://example.com/mcp"));
        assert_eq!(json["checks"].is_object(), true);
        assert_eq!(json["checks"]["mcp.config"]["id"], "mcp.config");
        assert_eq!(
            json["checks"]["mcp.config"]["details"]["OPENAI_API_KEY"],
            "<redacted>"
        );
        assert_eq!(
            json["checks"]["mcp.config"]["details"]["duplicate"],
            serde_json::json!(["one", "two"])
        );
        assert_eq!(
            json["checks"]["mcp.config"]["notes"],
            serde_json::json!(["freeform note"])
        );
        assert_eq!(
            json["checks"]["mcp.config"]["issues"][0]["measured"],
            "https://example.com/mcp"
        );
        assert_eq!(
            json["checks"]["mcp.config"]["issues"][0]["remedy"],
            "Check https://example.com/help."
        );
    }

    #[tokio::test]
    async fn mcp_check_ignores_disabled_servers() {
        let disabled_server: McpServerConfig = toml::from_str(
            r#"
                url = "http://127.0.0.1:9/mcp"
                enabled = false
                required = true
                bearer_token_env_var = "CODEX_DOCTOR_DISABLED_MCP_TOKEN"
            "#,
        )
        .expect("should deserialize disabled MCP config");
        let servers = HashMap::from([("disabled".to_string(), disabled_server)]);

        let check = mcp_check_from_servers(&servers).await;

        assert_eq!(check.status, CheckStatus::Ok);
        assert_eq!(check.summary, "MCP configuration is locally consistent");
        assert!(check.details.contains(&"disabled servers: 1".to_string()));
        assert!(
            check
                .details
                .iter()
                .all(|detail| !detail.contains("CODEX_DOCTOR_DISABLED_MCP_TOKEN"))
        );
        assert!(
            check
                .details
                .iter()
                .all(|detail| !detail.contains("reachability failed"))
        );
    }

    #[tokio::test]
    async fn mcp_check_warns_for_optional_http_reachability() {
        let optional_server: McpServerConfig = toml::from_str(
            r#"
                url = "http://127.0.0.1:9/mcp"
            "#,
        )
        .expect("should deserialize optional MCP config");
        let servers = HashMap::from([("optional".to_string(), optional_server)]);

        let check = mcp_check_from_servers(&servers).await;

        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(check.summary, "MCP configuration has optional issues");
        assert!(
            check
                .details
                .iter()
                .any(|detail| detail.contains("optional reachability failed: optional:"))
        );
    }

    #[tokio::test]
    async fn mcp_check_fails_required_remote_stdio_env_var() {
        let command = toml::Value::String(
            std::env::current_exe()
                .expect("current exe")
                .to_string_lossy()
                .into_owned(),
        );
        let required_server: McpServerConfig = toml::from_str(&format!(
            r#"
                command = {command}
                required = true
                env_vars = [{{ name = "REMOTE_ONLY_TOKEN", source = "remote" }}]
            "#,
        ))
        .expect("should deserialize required MCP config");
        let servers = HashMap::from([("required".to_string(), required_server)]);

        let check = mcp_check_from_servers(&servers).await;

        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.details.iter().any(|detail| {
            detail.contains(
                "required: env_vars entry `REMOTE_ONLY_TOKEN` uses source `remote`, which requires remote MCP stdio",
            )
        }));
    }

    #[test]
    fn provider_specific_auth_allows_non_openai_provider_without_env_key() {
        let check = provider_specific_auth_check(
            /*requires_openai_auth*/ false,
            /*provider_env_key*/ None,
            /*provider_env_key_instructions*/ None,
            Vec::new(),
            |_| false,
        )
        .expect("non-OpenAI provider should produce a provider-specific check");

        assert_eq!(check.status, CheckStatus::Ok);
        assert_eq!(
            check.summary,
            "OpenAI auth is not required for the active model provider"
        );
    }

    #[test]
    fn provider_specific_auth_fails_when_provider_env_key_is_missing() {
        let check = provider_specific_auth_check(
            /*requires_openai_auth*/ false,
            Some("PROVIDER_API_KEY"),
            Some("Set PROVIDER_API_KEY before running Codex."),
            Vec::new(),
            |_| false,
        )
        .expect("non-OpenAI provider should produce a provider-specific check");

        assert_eq!(check.status, CheckStatus::Fail);
        assert_eq!(
            check.summary,
            "active model provider auth env var is missing"
        );
        assert_eq!(
            check.remediation,
            Some("Set PROVIDER_API_KEY before running Codex.".to_string())
        );
    }

    #[test]
    fn stored_auth_validation_rejects_missing_api_key() {
        let auth = AuthDotJson {
            auth_mode: Some(codex_app_server_protocol::AuthMode::ApiKey),
            openai_api_key: None,
            tokens: None,
            last_refresh: None,
            agent_identity: None,
        };

        assert_eq!(
            stored_auth_issues(&auth, |_| false),
            vec!["API key auth is missing an API key"]
        );
        assert!(stored_auth_issues(&auth, |name| name == OPENAI_API_KEY_ENV_VAR).is_empty());
    }

    #[test]
    fn stored_auth_validation_rejects_missing_chatgpt_tokens() {
        let auth = AuthDotJson {
            auth_mode: None,
            openai_api_key: None,
            tokens: None,
            last_refresh: None,
            agent_identity: None,
        };

        assert_eq!(
            stored_auth_issues(&auth, |_| false),
            vec![
                "ChatGPT auth is missing token data",
                "ChatGPT auth is missing refresh metadata",
            ]
        );
    }

    #[test]
    fn provider_reachability_mode_uses_api_key_auth() {
        let api_key_auth = AuthDotJson {
            auth_mode: Some(codex_app_server_protocol::AuthMode::ApiKey),
            openai_api_key: Some("sk-test".to_string()),
            tokens: None,
            last_refresh: None,
            agent_identity: None,
        };

        assert_eq!(
            provider_auth_reachability_mode_from_auth(
                /*requires_openai_auth*/ true,
                |_| false,
                Some(&api_key_auth),
            ),
            ProviderAuthReachabilityMode::ApiKey
        );
        assert_eq!(
            provider_auth_reachability_mode_from_auth(
                /*requires_openai_auth*/ true,
                |name| name == OPENAI_API_KEY_ENV_VAR,
                /*stored_auth*/ None,
            ),
            ProviderAuthReachabilityMode::ApiKey
        );
    }

    #[test]
    fn provider_reachability_uses_active_provider_endpoint() {
        assert_eq!(
            provider_reachability_plan_from_parts(
                ProviderAuthReachabilityMode::NotRequired,
                "azure",
                "azure",
                Some("https://example.openai.azure.com/openai/v1"),
                /*provider_query_params*/ None,
                /*is_amazon_bedrock*/ false,
                "https://chatgpt.com/backend-api/",
            ),
            ReachabilityPlan {
                description: "provider auth".to_string(),
                endpoints: vec![ReachabilityEndpoint {
                    label: "azure API".to_string(),
                    url: "https://example.openai.azure.com/openai/v1".to_string(),
                    required: true,
                    route_probe_url: None,
                }],
            }
        );
    }

    #[test]
    fn provider_reachability_adds_models_route_probe_for_openai_compatible_base_urls() {
        let query_params = HashMap::from([("api-version".to_string(), "2026-01-01".to_string())]);

        assert_eq!(
            provider_reachability_plan_from_parts(
                ProviderAuthReachabilityMode::NotRequired,
                "custom",
                "Custom",
                Some("https://example.com/openai/v1/"),
                Some(&query_params),
                /*is_amazon_bedrock*/ false,
                "https://chatgpt.com/backend-api/",
            ),
            ReachabilityPlan {
                description: "provider auth".to_string(),
                endpoints: vec![ReachabilityEndpoint {
                    label: "custom API".to_string(),
                    url: "https://example.com/openai/v1/".to_string(),
                    required: true,
                    route_probe_url: Some(
                        "https://example.com/openai/v1/models?api-version=2026-01-01".to_string()
                    ),
                }],
            }
        );
    }

    #[test]
    fn provider_reachability_skips_route_probe_for_bedrock() {
        let plan = provider_reachability_plan_from_parts(
            ProviderAuthReachabilityMode::NotRequired,
            "amazon-bedrock",
            "Amazon Bedrock",
            Some("https://bedrock-runtime.us-east-1.amazonaws.com/openai/v1"),
            /*provider_query_params*/ None,
            /*is_amazon_bedrock*/ true,
            "https://chatgpt.com/backend-api/",
        );

        assert_eq!(plan.endpoints[0].route_probe_url, None);
    }

    #[test]
    fn provider_reachability_api_key_does_not_require_chatgpt() {
        let plan = provider_reachability_plan_from_parts(
            ProviderAuthReachabilityMode::ApiKey,
            "openai",
            "OpenAI",
            /*provider_base_url*/ None,
            /*provider_query_params*/ None,
            /*is_amazon_bedrock*/ false,
            "https://chatgpt.com/backend-api/",
        );

        assert_eq!(
            plan.endpoints,
            vec![ReachabilityEndpoint {
                label: "openai API".to_string(),
                url: "https://api.openai.com/v1".to_string(),
                required: true,
                route_probe_url: Some("https://api.openai.com/v1/models".to_string()),
            }]
        );
    }

    #[test]
    fn provider_reachability_outcome_reports_required_failures() {
        assert_eq!(
            provider_reachability_outcome(/*required_failures*/ 0, /*warnings*/ 1,),
            (
                CheckStatus::Warning,
                "provider endpoint checks returned warnings",
            )
        );
        assert_eq!(
            provider_reachability_outcome(/*required_failures*/ 1, /*warnings*/ 0,),
            (
                CheckStatus::Fail,
                "one or more required provider endpoints are unreachable over HTTP",
            )
        );
    }

    #[tokio::test]
    async fn provider_reachability_route_404_fails_bad_base_url_path() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener address");
        let server = std::thread::spawn(move || {
            respond_once(
                &listener,
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            respond_once(
                &listener,
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        });
        let plan = provider_reachability_plan_from_parts(
            ProviderAuthReachabilityMode::ApiKey,
            "openai",
            "OpenAI",
            Some(&format!("http://{addr}/xxxx")),
            /*provider_query_params*/ None,
            /*is_amazon_bedrock*/ false,
            "https://chatgpt.com/backend-api/",
        );

        let check = provider_reachability_check(plan).await;
        server.join().expect("probe server thread should finish");

        assert_eq!(check.status, CheckStatus::Fail);
        assert!(
            check
                .details
                .iter()
                .any(|detail| detail.contains("route probe:") && detail.contains("HTTP 404"))
        );
        assert_eq!(check.issues.len(), 1);
        assert_eq!(
            check.issues[0].remedy.as_deref(),
            Some("Set base_url to the provider API root, for example https://api.openai.com/v1")
        );
    }

    #[tokio::test]
    async fn provider_reachability_route_401_keeps_reachability_ok() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener address");
        let server = std::thread::spawn(move || {
            respond_once(
                &listener,
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            respond_once(
                &listener,
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        });
        let plan = provider_reachability_plan_from_parts(
            ProviderAuthReachabilityMode::ApiKey,
            "openai",
            "OpenAI",
            Some(&format!("http://{addr}/v1")),
            /*provider_query_params*/ None,
            /*is_amazon_bedrock*/ false,
            "https://chatgpt.com/backend-api/",
        );

        let check = provider_reachability_check(plan).await;
        server.join().expect("probe server thread should finish");

        assert_eq!(check.status, CheckStatus::Ok);
        assert!(
            check
                .details
                .iter()
                .any(|detail| detail.contains("route exists (HTTP 401)"))
        );
    }

    #[test]
    fn collect_rollout_stats_counts_nested_rollout_files() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let nested = temp
            .path()
            .join("sessions")
            .join("2026")
            .join("05")
            .join("13");
        std::fs::create_dir_all(&nested).expect("create nested rollout dir");
        std::fs::write(
            nested.join("rollout-2026-05-13T00-00-00-test.jsonl"),
            "12345",
        )
        .expect("write rollout file");
        std::fs::write(nested.join("not-a-rollout.jsonl"), "ignored").expect("write ignored jsonl");

        let stats = collect_rollout_stats(&temp.path().join("sessions"));

        assert_eq!(stats.files, 1);
        assert_eq!(stats.total_bytes, 5);
        assert_eq!(stats.average_bytes(), 5);
        assert_eq!(stats.error, None);
    }

    #[tokio::test]
    async fn http_probe_treats_http_status_as_reachable() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept probe request");
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("write response");
        });

        let status = http_probe_url(&format!("http://{addr}/mcp")).await;
        server.join().expect("probe server thread should finish");

        assert_eq!(status, Ok("HTTP 405".to_string()));
    }

    #[tokio::test]
    async fn mcp_http_probe_falls_back_to_get_when_head_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener address");
        let server = std::thread::spawn(move || {
            let (mut head_stream, _) = listener.accept().expect("accept HEAD probe request");
            let head = std::thread::spawn(move || {
                let mut request = [0; 1024];
                let _ = head_stream.read(&mut request);
                std::thread::sleep(Duration::from_millis(50));
            });

            let (mut get_stream, _) = listener.accept().expect("accept GET probe request");
            let mut request = [0; 1024];
            let _ = get_stream.read(&mut request);
            get_stream
                .write_all(
                    b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("write response");
            head.join().expect("HEAD holder should finish");
        });

        let status = mcp_http_probe_url_with_timeout(
            &format!("http://{addr}/mcp"),
            Duration::from_millis(10),
        )
        .await;
        server.join().expect("probe server thread should finish");

        assert_eq!(status, Ok("HTTP 405".to_string()));
    }

    #[tokio::test]
    async fn mcp_check_fails_required_missing_stdio_command() {
        let required_server: McpServerConfig = toml::from_str(
            r#"
                command = "definitely-missing-codex-doctor-mcp"
                required = true
            "#,
        )
        .expect("should deserialize required MCP config");
        let servers = HashMap::from([("required".to_string(), required_server)]);

        let check = mcp_check_from_servers(&servers).await;

        assert_eq!(check.status, CheckStatus::Fail);
        assert_eq!(
            check.summary,
            "MCP configuration has failing required inputs or reachability"
        );
        assert!(check.details.iter().any(|detail| {
            detail.contains(
                "required: stdio command \"definitely-missing-codex-doctor-mcp\" is not resolvable",
            )
        }));
    }

    #[cfg(unix)]
    #[test]
    fn read_probe_file_rejects_unreadable_file() {
        use std::os::unix::fs::PermissionsExt;

        let file = tempfile::NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), "cert").expect("write temp file");
        let mut permissions = std::fs::metadata(file.path())
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o000);
        std::fs::set_permissions(file.path(), permissions).expect("remove read permissions");

        let result = read_probe_file(file.path());

        let mut permissions = std::fs::metadata(file.path())
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(file.path(), permissions).expect("restore read permissions");
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn executable_path_exists_rejects_non_executable_file() {
        use std::os::unix::fs::PermissionsExt;

        let file = tempfile::NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), "#!/bin/sh\n").expect("write temp file");
        let mut permissions = std::fs::metadata(file.path())
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(file.path(), permissions).expect("set non-executable mode");

        let result = executable_path_exists(file.path());

        assert!(result.is_err());
        let mut permissions = std::fs::metadata(file.path())
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(file.path(), permissions).expect("set executable mode");
        assert_eq!(executable_path_exists(file.path()), Ok(()));
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

    fn terminal_inputs() -> TerminalCheckInputs {
        TerminalCheckInputs {
            info: TerminalInfo {
                name: TerminalName::Unknown,
                term_program: None,
                version: None,
                term: Some("xterm-256color".to_string()),
                multiplexer: None,
            },
            env: BTreeMap::from([("TERM".to_string(), "xterm-256color".to_string())]),
            present_env: BTreeSet::from(["TERM".to_string()]),
            no_color_flag: false,
            stdin_is_terminal: true,
            stdout_is_terminal: true,
            stderr_is_terminal: true,
            stream_supports_color: true,
            terminal_size: Ok((120, 40)),
            tmux_details: Vec::new(),
        }
    }

    fn set_terminal_env(inputs: &mut TerminalCheckInputs, name: &str, value: &str) {
        inputs.present_env.insert(name.to_string());
        if value.is_empty() {
            inputs.env.remove(name);
        } else {
            inputs.env.insert(name.to_string(), value.to_string());
        }
    }

    #[test]
    fn terminal_check_warns_for_dumb_terminal() {
        let mut inputs = terminal_inputs();
        inputs.info.name = TerminalName::Dumb;
        inputs.info.term = Some("dumb".to_string());
        set_terminal_env(&mut inputs, "TERM", "dumb");

        let check = terminal_check_from_inputs(inputs);

        assert_eq!(check.status, CheckStatus::Fail);
        assert_eq!(
            check.summary,
            "TERM=dumb - colors and cursor control are disabled"
        );
        assert_eq!(check.issues.len(), 1);
        assert_eq!(
            check.issues[0].remedy.as_deref(),
            Some("set TERM to a real value, for example xterm-256color")
        );
    }

    #[test]
    fn terminal_check_warns_for_narrow_terminal() {
        let mut inputs = terminal_inputs();
        inputs.terminal_size = Ok((79, 24));

        let check = terminal_check_from_inputs(inputs);

        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(
            check.summary,
            "width 79 cols - output may wrap (recommended >=80)"
        );
        assert_eq!(check.issues[0].expected.as_deref(), Some(">= 80 columns"));
        assert_eq!(
            check.issues[0].remedy.as_deref(),
            Some("resize the window to at least 80 columns")
        );
    }

    #[test]
    fn terminal_check_warns_for_declared_narrow_terminal() {
        let mut inputs = terminal_inputs();
        set_terminal_env(&mut inputs, "COLUMNS", "60");

        let check = terminal_check_from_inputs(inputs);

        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(
            check.summary,
            "COLUMNS=60 - output may wrap (recommended >=80)"
        );
        assert!(check.details.contains(&"COLUMNS: 60".to_string()));
        assert_eq!(check.issues[0].fields, vec!["COLUMNS".to_string()]);
    }

    #[test]
    fn terminal_check_warns_for_non_utf8_locale() {
        let mut inputs = terminal_inputs();
        set_terminal_env(&mut inputs, "LANG", "C");

        let check = terminal_check_from_inputs(inputs);

        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(
            check.summary,
            "locale is not UTF-8 - unicode glyphs may render incorrectly"
        );
        assert!(check.details.contains(&"effective locale: C".to_string()));
        assert_eq!(
            check.issues[0].remedy.as_deref(),
            Some("export LANG=en_US.UTF-8 or another UTF-8 locale")
        );
    }

    #[test]
    fn terminal_check_warns_for_unreadable_terminfo_path() {
        let tempdir = tempfile::tempdir().expect("create tempdir");
        let missing = tempdir.path().join("missing-terminfo");
        let mut inputs = terminal_inputs();
        set_terminal_env(&mut inputs, "TERMINFO", &missing.to_string_lossy());

        let check = terminal_check_from_inputs(inputs);

        assert_eq!(check.status, CheckStatus::Fail);
        assert_eq!(
            check.summary,
            "TERMINFO unreadable - terminal capabilities are unknown"
        );
        assert!(
            check
                .details
                .iter()
                .any(|detail| detail.starts_with("TERMINFO: ") && detail.ends_with(" (missing)"))
        );
        assert_eq!(
            check.issues[0].remedy.as_deref(),
            Some("check that $TERMINFO points to a readable directory")
        );
    }

    #[test]
    fn terminal_check_reports_remote_indicators_as_present_only() {
        let mut inputs = terminal_inputs();
        set_terminal_env(&mut inputs, "SSH_CONNECTION", "10.0.0.1 1 10.0.0.2 22");

        let check = terminal_check_from_inputs(inputs);

        assert!(
            check
                .details
                .contains(&"SSH_CONNECTION: present".to_string())
        );
        assert!(
            !check
                .details
                .iter()
                .any(|detail| detail.contains("10.0.0.1"))
        );
    }

    #[test]
    fn terminal_check_keeps_tmux_probe_failures_non_fatal() {
        let mut inputs = terminal_inputs();
        inputs.info.multiplexer = Some(Multiplexer::Tmux { version: None });

        let check = terminal_check_from_inputs(inputs);

        assert_eq!(check.status, CheckStatus::Ok);
        assert_eq!(check.summary, "terminal metadata was detected");
    }

    #[test]
    fn color_output_summary_reports_disabled_reasons() {
        let mut inputs = terminal_inputs();
        inputs.no_color_flag = true;
        assert_eq!(color_output_summary(&inputs), "disabled (--no-color)");

        inputs = terminal_inputs();
        set_terminal_env(&mut inputs, "NO_COLOR", "");
        assert_eq!(color_output_summary(&inputs), "disabled (NO_COLOR)");

        inputs = terminal_inputs();
        inputs.info.term = Some("dumb".to_string());
        set_terminal_env(&mut inputs, "TERM", "dumb");
        assert_eq!(color_output_summary(&inputs), "disabled (TERM=dumb)");

        inputs = terminal_inputs();
        inputs.stdout_is_terminal = false;
        assert_eq!(
            color_output_summary(&inputs),
            "disabled (stdout is not a terminal)"
        );
    }
}
