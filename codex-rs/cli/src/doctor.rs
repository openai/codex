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
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::future::Future;
use std::io::IsTerminal;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use clap::Parser;
use codex_arg0::Arg0DispatchPaths;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_install_context::InstallContext;
use codex_install_context::StandalonePlatform;
use codex_login::AuthDotJson;
use codex_login::CODEX_ACCESS_TOKEN_ENV_VAR;
use codex_login::CODEX_API_KEY_ENV_VAR;
use codex_login::OPENAI_API_KEY_ENV_VAR;
use codex_login::default_client::build_reqwest_client;
use codex_login::load_auth_dot_json;
use codex_protocol::protocol::AskForApproval;
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
use output::redact_detail;
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
/// Summaries are safe for the default human view. Details may include local
/// paths or command output and are therefore shown only with --verbose in
/// human mode, while JSON consumers receive the full redacted report.
#[derive(Clone, Debug, Serialize)]
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
        println!(
            "{}",
            serde_json::to_string_pretty(&redacted_report(&report))?
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
            checks.push(timed_check_async(|| mcp_check(config)).await);
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

    let openai_reachability_mode = config_result
        .as_ref()
        .map(openai_reachability_mode)
        .unwrap_or(OpenAiReachabilityMode::Chatgpt);
    checks.push(timed_check_async(|| openai_reachability_check(openai_reachability_mode)).await);

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

    Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, overrides)
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

fn redacted_report(report: &DoctorReport) -> DoctorReport {
    let mut redacted = report.clone();
    for check in &mut redacted.checks {
        check.details = check
            .details
            .iter()
            .map(|detail| redact_detail(detail))
            .collect();
        check.remediation = check.remediation.as_deref().map(redact_detail);
    }
    redacted
}

fn timed_check(f: impl FnOnce() -> DoctorCheck) -> DoctorCheck {
    let start = Instant::now();
    let mut check = f();
    check.duration_ms = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    check
}

async fn timed_check_async<F, Fut>(f: F) -> DoctorCheck
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = DoctorCheck>,
{
    let start = Instant::now();
    let mut check = f().await;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenAiReachabilityMode {
    NotRequired,
    ApiKey,
    Chatgpt,
}

impl OpenAiReachabilityMode {
    fn description(self) -> &'static str {
        match self {
            Self::NotRequired => "not required by active model provider",
            Self::ApiKey => "API key auth",
            Self::Chatgpt => "ChatGPT auth",
        }
    }
}

fn openai_reachability_mode(config: &Config) -> OpenAiReachabilityMode {
    let stored_auth =
        load_auth_dot_json(&config.codex_home, config.cli_auth_credentials_store_mode)
            .ok()
            .flatten();
    openai_reachability_mode_from_auth(
        config.model_provider.requires_openai_auth,
        env_var_present,
        stored_auth.as_ref(),
    )
}

fn openai_reachability_mode_from_auth(
    requires_openai_auth: bool,
    env_var_present: impl Fn(&str) -> bool,
    stored_auth: Option<&AuthDotJson>,
) -> OpenAiReachabilityMode {
    if !requires_openai_auth {
        return OpenAiReachabilityMode::NotRequired;
    }
    if env_var_present(OPENAI_API_KEY_ENV_VAR) || env_var_present(CODEX_API_KEY_ENV_VAR) {
        return OpenAiReachabilityMode::ApiKey;
    }
    if env_var_present(CODEX_ACCESS_TOKEN_ENV_VAR) {
        return OpenAiReachabilityMode::Chatgpt;
    }
    match stored_auth.map(stored_auth_mode_value) {
        Some(codex_app_server_protocol::AuthMode::ApiKey) => OpenAiReachabilityMode::ApiKey,
        Some(
            codex_app_server_protocol::AuthMode::Chatgpt
            | codex_app_server_protocol::AuthMode::ChatgptAuthTokens
            | codex_app_server_protocol::AuthMode::AgentIdentity,
        )
        | None => OpenAiReachabilityMode::Chatgpt,
    }
}

async fn openai_reachability_check(mode: OpenAiReachabilityMode) -> DoctorCheck {
    let endpoints = match mode {
        OpenAiReachabilityMode::ApiKey => vec![("https://api.openai.com/", true)],
        OpenAiReachabilityMode::Chatgpt => vec![
            ("https://api.openai.com/", true),
            ("https://chatgpt.com/", true),
        ],
        OpenAiReachabilityMode::NotRequired => vec![
            ("https://api.openai.com/", false),
            ("https://chatgpt.com/", false),
        ],
    };
    let mut details = vec![format!("reachability mode: {}", mode.description())];
    let mut failures = Vec::new();
    let mut optional_failures = Vec::new();
    for (url, required) in endpoints {
        match http_probe_url(url).await {
            Ok(status) => details.push(format!("{url}: reachable ({status})")),
            Err(err) => {
                let requirement = if required { "required" } else { "optional" };
                details.push(format!("{url}: {err} ({requirement})"));
                if required {
                    failures.push(url);
                } else {
                    optional_failures.push(url);
                }
            }
        }
    }

    let (status, summary) = openai_reachability_outcome(failures.len(), optional_failures.len());
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

fn openai_reachability_outcome(
    required_failures: usize,
    optional_failures: usize,
) -> (CheckStatus, &'static str) {
    match (required_failures, optional_failures) {
        (0, 0) => (CheckStatus::Ok, "OpenAI endpoints are reachable over HTTP"),
        (0, _) => (
            CheckStatus::Warning,
            "OpenAI endpoints are unreachable but not required by the active provider",
        ),
        (_, _) => (
            CheckStatus::Fail,
            "one or more required OpenAI endpoints are unreachable over HTTP",
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
    Ok(format!("HTTP {}", response.status().as_u16()))
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
    use std::io::Read;
    use std::io::Write;
    use std::net::TcpListener;

    use clap::Parser;
    use codex_protocol::config_types::SandboxMode;
    use pretty_assertions::assert_eq;

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
    fn redacted_report_sanitizes_json_details() {
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
                .remediation("Open https://user:pass@example.com/help?x=abc."),
            ],
        };

        let redacted = serde_json::to_string(&redacted_report(&report)).expect("serialize report");

        assert!(!redacted.contains("user:pass"));
        assert!(!redacted.contains("x=abc"));
        assert!(!redacted.contains("sk-live-secret"));
        assert!(redacted.contains("https://example.com/mcp"));
        assert!(redacted.contains("OPENAI_API_KEY: <redacted>"));
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
    fn openai_reachability_mode_uses_api_key_auth() {
        let api_key_auth = AuthDotJson {
            auth_mode: Some(codex_app_server_protocol::AuthMode::ApiKey),
            openai_api_key: Some("sk-test".to_string()),
            tokens: None,
            last_refresh: None,
            agent_identity: None,
        };

        assert_eq!(
            openai_reachability_mode_from_auth(
                /*requires_openai_auth*/ true,
                |_| false,
                Some(&api_key_auth),
            ),
            OpenAiReachabilityMode::ApiKey
        );
        assert_eq!(
            openai_reachability_mode_from_auth(
                /*requires_openai_auth*/ true,
                |name| name == OPENAI_API_KEY_ENV_VAR,
                /*stored_auth*/ None,
            ),
            OpenAiReachabilityMode::ApiKey
        );
    }

    #[test]
    fn openai_reachability_warns_when_openai_is_not_required() {
        assert_eq!(
            openai_reachability_outcome(/*required_failures*/ 0, /*optional_failures*/ 1,),
            (
                CheckStatus::Warning,
                "OpenAI endpoints are unreachable but not required by the active provider",
            )
        );
        assert_eq!(
            openai_reachability_outcome(/*required_failures*/ 1, /*optional_failures*/ 0,),
            (
                CheckStatus::Fail,
                "one or more required OpenAI endpoints are unreachable over HTTP",
            )
        );
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
}
