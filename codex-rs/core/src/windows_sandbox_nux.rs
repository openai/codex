//! Shared Windows sandbox NUX orchestration.
//!
//! This module owns the cross-surface workflow for the two-screen Windows
//! sandbox NUX:
//! - an initial "enable sandbox" screen
//! - a fallback screen shown after elevated setup failure
//!
//! The orchestration is transport-agnostic. Core does not know about app-server
//! JSON-RPC, TUI widgets, or any specific UI runtime. Instead, callers inject a
//! [`WindowsSandboxNuxHost`] implementation that provides two callbacks:
//! - request a user decision for the current prompt/screen
//! - receive async setup status updates
//!
//! This keeps workflow logic centralized in core while letting each surface map
//! host callbacks to its own IO model (for example, app-server server-requests/
//! notifications today, and a direct TUI adapter in the future).
//!
//! Responsibilities in this module:
//! - run the prompt/action loop
//! - execute elevated setup attempts (including retry path)
//! - execute unelevated legacy preflight path
//! - persist resulting sandbox config mode
//! - emit `codex.windows_sandbox.*` metrics with a surface tag
//! - emit setup status heartbeats through the injected host
//!
//! Non-goals:
//! - defining UI layout/copy for screens
//! - implementing transport-level protocols
//! - owning app-server request parsing/response shaping
//!
//! Current behavior note: async setup status updates are emitted for elevated
//! setup attempts. Unelevated legacy preflight currently runs as a single
//! operation without intermediate progress updates.

use crate::config::edit::ConfigEditsBuilder;
use crate::protocol::SandboxPolicy;
use crate::windows_sandbox;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsSandboxNuxSurface {
    Tui,
    CodexApp,
    Vscode,
}

impl WindowsSandboxNuxSurface {
    fn metric_tag(self) -> &'static str {
        match self {
            WindowsSandboxNuxSurface::Tui => "tui",
            WindowsSandboxNuxSurface::CodexApp => "codex_app",
            WindowsSandboxNuxSurface::Vscode => "vscode",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowsSandboxNuxPromptScreen {
    Enable,
    Fallback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsSandboxNuxPromptRequest {
    pub screen: WindowsSandboxNuxPromptScreen,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowsSandboxNuxPromptAction {
    SetupElevated,
    SetupUnelevated,
    RetryElevated,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowsSandboxSetupStatus {
    Started,
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsSandboxSetupStatusUpdate {
    pub status: WindowsSandboxSetupStatus,
    pub attempt: u32,
    pub elapsed_ms: i64,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct WindowsSandboxNuxRunParams {
    pub surface: WindowsSandboxNuxSurface,
    pub codex_home: PathBuf,
    pub active_profile: Option<String>,
    pub policy: SandboxPolicy,
    pub policy_cwd: PathBuf,
    pub command_cwd: PathBuf,
    pub env_map: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowsSandboxNuxOutcome {
    EnabledElevated,
    EnabledUnelevated,
    Quit,
    PromptUnavailable,
}

#[async_trait]
pub trait WindowsSandboxNuxHost: Send {
    async fn request_prompt(
        &mut self,
        request: WindowsSandboxNuxPromptRequest,
    ) -> Option<WindowsSandboxNuxPromptAction>;

    async fn on_setup_status(&mut self, update: WindowsSandboxSetupStatusUpdate);
}

async fn persist_windows_sandbox_mode(
    codex_home: &PathBuf,
    profile: Option<&str>,
    elevated_enabled: bool,
) -> anyhow::Result<()> {
    ConfigEditsBuilder::new(codex_home)
        .with_profile(profile)
        .set_windows_sandbox_mode(if elevated_enabled {
            "elevated"
        } else {
            "unelevated"
        })
        .clear_legacy_windows_sandbox_keys()
        .apply()
        .await
}

async fn emit_setup_status(
    host: &mut dyn WindowsSandboxNuxHost,
    status: WindowsSandboxSetupStatus,
    attempt: u32,
    started_at: Instant,
    failure_code: Option<String>,
    failure_message: Option<String>,
) {
    let elapsed_ms = i64::try_from(started_at.elapsed().as_millis()).unwrap_or(i64::MAX);
    host.on_setup_status(WindowsSandboxSetupStatusUpdate {
        status,
        attempt,
        elapsed_ms,
        failure_code,
        failure_message,
    })
    .await;
}

fn screen_metric_name(
    screen: &WindowsSandboxNuxPromptScreen,
    enable_metric: &'static str,
    fallback_metric: &'static str,
) -> &'static str {
    if *screen == WindowsSandboxNuxPromptScreen::Enable {
        enable_metric
    } else {
        fallback_metric
    }
}

async fn run_unelevated_setup(
    surface_tag: &str,
    codex_home: &PathBuf,
    active_profile: Option<&str>,
    policy: &SandboxPolicy,
    policy_cwd: &PathBuf,
    command_cwd: &PathBuf,
    env_map: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let policy_for_preflight = policy.clone();
    let policy_cwd_for_preflight = policy_cwd.clone();
    let command_cwd_for_preflight = command_cwd.clone();
    let env_for_preflight = env_map.clone();
    let codex_home_for_preflight = codex_home.clone();
    let preflight_result = tokio::task::spawn_blocking(move || {
        windows_sandbox::run_legacy_setup_preflight(
            &policy_for_preflight,
            policy_cwd_for_preflight.as_path(),
            command_cwd_for_preflight.as_path(),
            &env_for_preflight,
            codex_home_for_preflight.as_path(),
        )
    })
    .await;

    if let Ok(Ok(())) = preflight_result {
    } else {
        windows_sandbox::record_windows_sandbox_counter(
            "codex.windows_sandbox.legacy_setup_preflight_failed",
            surface_tag,
            &[],
        );
    }

    persist_windows_sandbox_mode(codex_home, active_profile, false).await
}

enum ElevatedAttemptOutcome {
    Success,
    Failed {
        fallback_code: Option<String>,
        fallback_message: Option<String>,
    },
}

async fn run_elevated_setup_attempt(
    host: &mut dyn WindowsSandboxNuxHost,
    surface_tag: &str,
    codex_home: &PathBuf,
    active_profile: Option<&str>,
    policy: &SandboxPolicy,
    policy_cwd: &PathBuf,
    command_cwd: &PathBuf,
    env_map: &HashMap<String, String>,
    attempt: u32,
) -> anyhow::Result<ElevatedAttemptOutcome> {
    let started_at = Instant::now();
    emit_setup_status(
        host,
        WindowsSandboxSetupStatus::Started,
        attempt,
        started_at,
        None,
        None,
    )
    .await;

    let policy_for_setup = policy.clone();
    let policy_cwd_for_setup = policy_cwd.clone();
    let command_cwd_for_setup = command_cwd.clone();
    let env_for_setup = env_map.clone();
    let codex_home_for_setup = codex_home.clone();
    let mut setup_task = tokio::task::spawn_blocking(move || {
        windows_sandbox::run_elevated_setup(
            &policy_for_setup,
            policy_cwd_for_setup.as_path(),
            command_cwd_for_setup.as_path(),
            &env_for_setup,
            codex_home_for_setup.as_path(),
        )
    });

    let mut progress = tokio::time::interval(Duration::from_secs(5));
    let setup_result = loop {
        tokio::select! {
            _ = progress.tick() => {
                emit_setup_status(
                    host,
                    WindowsSandboxSetupStatus::Running,
                    attempt,
                    started_at,
                    None,
                    None,
                ).await;
            }
            join_result = &mut setup_task => {
                break match join_result {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(err)) => Err(err),
                    Err(err) => Err(anyhow::Error::msg(format!("setup task failed: {err}"))),
                };
            }
        }
    };

    match setup_result {
        Ok(()) => {
            if let Err(err) = persist_windows_sandbox_mode(codex_home, active_profile, true).await {
                emit_setup_status(
                    host,
                    WindowsSandboxSetupStatus::Failed,
                    attempt,
                    started_at,
                    Some("configPersistFailed".to_string()),
                    Some(err.to_string()),
                )
                .await;
                return Err(err);
            }
            windows_sandbox::record_windows_sandbox_counter(
                "codex.windows_sandbox.elevated_setup_success",
                surface_tag,
                &[],
            );
            windows_sandbox::record_windows_sandbox_histogram(
                "codex.windows_sandbox.elevated_setup_duration_ms",
                i64::try_from(started_at.elapsed().as_millis()).unwrap_or(i64::MAX),
                surface_tag,
                &[("result", "success")],
            );
            emit_setup_status(
                host,
                WindowsSandboxSetupStatus::Completed,
                attempt,
                started_at,
                None,
                None,
            )
            .await;
            Ok(ElevatedAttemptOutcome::Success)
        }
        Err(err) => {
            let (fallback_code, fallback_message) =
                windows_sandbox::elevated_setup_failure_details(&err)
                    .map_or((None, None), |(code, message)| (Some(code), Some(message)));
            let mut tags: Vec<(&str, &str)> = Vec::new();
            if let Some(code) = fallback_code.as_deref() {
                tags.push(("code", code));
            }
            if let Some(message) = fallback_message.as_deref() {
                tags.push(("message", message));
            }
            windows_sandbox::record_windows_sandbox_counter(
                windows_sandbox::elevated_setup_failure_metric_name(&err),
                surface_tag,
                &tags,
            );
            windows_sandbox::record_windows_sandbox_histogram(
                "codex.windows_sandbox.elevated_setup_duration_ms",
                i64::try_from(started_at.elapsed().as_millis()).unwrap_or(i64::MAX),
                surface_tag,
                &[("result", "failure")],
            );
            emit_setup_status(
                host,
                WindowsSandboxSetupStatus::Failed,
                attempt,
                started_at,
                fallback_code.clone(),
                fallback_message.clone(),
            )
            .await;
            Ok(ElevatedAttemptOutcome::Failed {
                fallback_code,
                fallback_message,
            })
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub async fn run_windows_sandbox_nux(
    _host: &mut dyn WindowsSandboxNuxHost,
    _params: WindowsSandboxNuxRunParams,
) -> anyhow::Result<WindowsSandboxNuxOutcome> {
    anyhow::bail!("windows sandbox nux is only supported on Windows")
}

#[cfg(target_os = "windows")]
pub async fn run_windows_sandbox_nux(
    host: &mut dyn WindowsSandboxNuxHost,
    params: WindowsSandboxNuxRunParams,
) -> anyhow::Result<WindowsSandboxNuxOutcome> {
    let WindowsSandboxNuxRunParams {
        surface,
        codex_home,
        active_profile,
        policy,
        policy_cwd,
        command_cwd,
        env_map,
    } = params;

    let surface_tag = surface.metric_tag();
    let mut prompt_screen = WindowsSandboxNuxPromptScreen::Enable;
    let mut fallback_code: Option<String> = None;
    let mut fallback_message: Option<String> = None;
    let mut attempt: u32 = 0;

    loop {
        let shown_metric = screen_metric_name(
            &prompt_screen,
            "codex.windows_sandbox.elevated_prompt_shown",
            "codex.windows_sandbox.fallback_prompt_shown",
        );
        windows_sandbox::record_windows_sandbox_counter(shown_metric, surface_tag, &[]);

        let action = host
            .request_prompt(WindowsSandboxNuxPromptRequest {
                screen: prompt_screen.clone(),
                failure_code: fallback_code.clone(),
                failure_message: fallback_message.clone(),
            })
            .await;
        let Some(action) = action else {
            return Ok(WindowsSandboxNuxOutcome::PromptUnavailable);
        };

        match action {
            WindowsSandboxNuxPromptAction::Quit => {
                let quit_metric = screen_metric_name(
                    &prompt_screen,
                    "codex.windows_sandbox.elevated_prompt_quit",
                    "codex.windows_sandbox.fallback_prompt_quit",
                );
                windows_sandbox::record_windows_sandbox_counter(quit_metric, surface_tag, &[]);
                return Ok(WindowsSandboxNuxOutcome::Quit);
            }
            WindowsSandboxNuxPromptAction::SetupUnelevated => {
                let use_legacy_metric = screen_metric_name(
                    &prompt_screen,
                    "codex.windows_sandbox.elevated_prompt_use_legacy",
                    "codex.windows_sandbox.fallback_use_legacy",
                );
                windows_sandbox::record_windows_sandbox_counter(
                    use_legacy_metric,
                    surface_tag,
                    &[],
                );

                run_unelevated_setup(
                    surface_tag,
                    &codex_home,
                    active_profile.as_deref(),
                    &policy,
                    &policy_cwd,
                    &command_cwd,
                    &env_map,
                )
                .await?;
                return Ok(WindowsSandboxNuxOutcome::EnabledUnelevated);
            }
            WindowsSandboxNuxPromptAction::SetupElevated
            | WindowsSandboxNuxPromptAction::RetryElevated => {
                let action_metric = if action == WindowsSandboxNuxPromptAction::SetupElevated {
                    "codex.windows_sandbox.elevated_prompt_accept"
                } else {
                    "codex.windows_sandbox.fallback_retry_elevated"
                };
                windows_sandbox::record_windows_sandbox_counter(action_metric, surface_tag, &[]);

                if windows_sandbox::sandbox_setup_is_complete(codex_home.as_path()) {
                    persist_windows_sandbox_mode(&codex_home, active_profile.as_deref(), true)
                        .await?;
                    windows_sandbox::record_windows_sandbox_counter(
                        "codex.windows_sandbox.elevated_setup_success",
                        surface_tag,
                        &[],
                    );
                    return Ok(WindowsSandboxNuxOutcome::EnabledElevated);
                }

                attempt = attempt.saturating_add(1);
                let outcome = run_elevated_setup_attempt(
                    host,
                    surface_tag,
                    &codex_home,
                    active_profile.as_deref(),
                    &policy,
                    &policy_cwd,
                    &command_cwd,
                    &env_map,
                    attempt,
                )
                .await;
                match outcome? {
                    ElevatedAttemptOutcome::Success => {
                        return Ok(WindowsSandboxNuxOutcome::EnabledElevated);
                    }
                    ElevatedAttemptOutcome::Failed {
                        fallback_code: new_fallback_code,
                        fallback_message: new_fallback_message,
                    } => {
                        fallback_code = new_fallback_code;
                        fallback_message = new_fallback_message;
                        prompt_screen = WindowsSandboxNuxPromptScreen::Fallback;
                    }
                }
            }
        }
    }
}
