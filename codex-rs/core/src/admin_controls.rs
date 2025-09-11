use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

pub const ADMIN_DANGEROUS_SANDBOX_DISABLED_MESSAGE: &str = "Running Codex with --dangerously-bypass-approvals-and-sandbox or --sandbox danger-full-access has been disabled by your administrator. Please contact your system administrator or try: codex --full-auto";

pub const ADMIN_DANGEROUS_SANDBOX_DISABLED_PROMPT_LINES: &[&str] = &[
    "Running Codex with --dangerously-bypass-approvals-and-sandbox or",
    "--sandbox danger-full-access has been disabled by your administrator.",
    "\nPlease contact your system administrator or try with sandboxed mode:",
    "codex --full-auto",
];

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AdminConfigToml {
    #[serde(default)]
    pub disallow_dangerous_sandbox: Option<bool>,

    #[serde(default)]
    pub disallow_dangerously_bypass_approvals_and_sandbox: Option<bool>,

    #[serde(default)]
    pub allow_danger_with_reason: Option<bool>,

    #[serde(default)]
    pub audit: Option<AdminAuditToml>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AdminAuditToml {
    pub endpoint: Option<String>,

    #[serde(default)]
    pub dangerous_sandbox: Option<bool>,

    #[serde(default)]
    pub all_commands: Option<bool>,

    #[serde(default)]
    pub audit_log_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdminControls {
    pub disallow_dangerous_sandbox: bool,
    pub disallow_dangerously_bypass_approvals_and_sandbox: bool,
    pub allow_danger_with_reason: bool,
    pub audit: Option<AdminAudit>,
}

impl AdminControls {
    pub fn from_toml(admin: Option<AdminConfigToml>) -> Self {
        let mut controls = AdminControls::default();

        if let Some(section) = admin {
            if let Some(value) = section.disallow_dangerous_sandbox {
                controls.disallow_dangerous_sandbox = value;
            }

            if let Some(value) = section.disallow_dangerously_bypass_approvals_and_sandbox {
                controls.disallow_dangerously_bypass_approvals_and_sandbox = value;
            }

            if let Some(value) = section.allow_danger_with_reason {
                controls.allow_danger_with_reason = value;
            }

            controls.audit = section.audit.and_then(AdminAudit::from_toml);
        }

        controls
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminAudit {
    pub endpoint: String,
    pub events: AdminAuditEvents,
    pub audit_log_file: Option<PathBuf>,
}

impl AdminAudit {
    fn from_toml(config: AdminAuditToml) -> Option<Self> {
        let AdminAuditToml {
            endpoint,
            dangerous_sandbox,
            all_commands,
            audit_log_file,
        } = config;

        let endpoint = endpoint?.trim().to_string();
        if endpoint.is_empty() {
            return None;
        }

        let events = AdminAuditEvents {
            dangerous_sandbox: dangerous_sandbox.unwrap_or(false),
            all_commands: all_commands.unwrap_or(false),
        };

        Some(Self {
            endpoint,
            events,
            audit_log_file: audit_log_file
                .as_deref()
                .and_then(resolve_admin_audit_log_file),
        })
    }
}

fn resolve_admin_audit_log_file(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(home) = trimmed.strip_prefix("~/") {
        let mut expanded = dirs::home_dir()?;
        expanded.push(home);
        Some(expanded)
    } else if trimmed == "~" {
        dirs::home_dir()
    } else {
        Some(PathBuf::from(trimmed))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdminAuditEvents {
    pub dangerous_sandbox: bool,
    pub all_commands: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AdminDangerPrompt {
    pub sandbox: bool,
    pub dangerously_bypass: bool,
}

impl AdminDangerPrompt {
    pub fn needs_prompt(&self) -> bool {
        self.sandbox || self.dangerously_bypass
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AdminAuditContext<'a> {
    pub sandbox_policy: &'a SandboxPolicy,
    pub approval_policy: &'a AskForApproval,
    pub cwd: &'a Path,
    pub command: Option<&'a [String]>,
    pub dangerously_bypass_requested: bool,
    pub dangerous_mode_justification: Option<&'a str>,
    pub record_command_event: bool,
}

#[derive(Debug, Clone, Copy)]
enum AdminAuditEventKind {
    CommandInvoked,
    DangerousSandbox,
}

impl AdminAuditEventKind {
    fn label(self) -> &'static str {
        match self {
            Self::CommandInvoked => "command-invoked",
            Self::DangerousSandbox => "dangerous-sandbox",
        }
    }
}

#[derive(Serialize)]
struct AdminAuditPayload<'a> {
    event: &'a str,
    sandbox_mode: &'a str,
    username: String,
    dangerously_bypass_requested: bool,
    timestamp: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    justification: Option<&'a str>,
    approval_policy: &'a str,
    cwd: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a [String]>,
}

#[derive(Debug, Clone, Copy)]
struct AdminAuditEventDetails<'a> {
    sandbox_mode: &'a str,
    dangerously_bypass_requested: bool,
    justification: Option<&'a str>,
    approval_policy: &'a str,
    cwd: &'a str,
    command: Option<&'a [String]>,
}

pub async fn maybe_post_admin_audit_events(
    admin_controls: &AdminControls,
    context: AdminAuditContext<'_>,
) {
    let Some(audit) = admin_controls.audit.as_ref() else {
        return;
    };

    if !audit.events.all_commands && !audit.events.dangerous_sandbox {
        return;
    }

    let sandbox_mode = context.sandbox_policy.to_string();
    let approval_policy_display = context.approval_policy.to_string();
    let cwd_display = context.cwd.display().to_string();
    let command_args = context.command;
    let client = Client::new();

    let event_details = AdminAuditEventDetails {
        sandbox_mode: &sandbox_mode,
        dangerously_bypass_requested: context.dangerously_bypass_requested,
        justification: context.dangerous_mode_justification,
        approval_policy: &approval_policy_display,
        cwd: &cwd_display,
        command: command_args,
    };

    if audit.events.all_commands && context.record_command_event {
        post_admin_audit_event(
            &client,
            &audit.endpoint,
            audit.audit_log_file.as_deref(),
            AdminAuditEventKind::CommandInvoked,
            event_details,
        )
        .await;
    }

    let dangerous_requested = context.dangerously_bypass_requested
        || matches!(context.sandbox_policy, SandboxPolicy::DangerFullAccess);

    if dangerous_requested && audit.events.dangerous_sandbox {
        post_admin_audit_event(
            &client,
            &audit.endpoint,
            audit.audit_log_file.as_deref(),
            AdminAuditEventKind::DangerousSandbox,
            event_details,
        )
        .await;
    }
}

pub async fn audit_admin_run_with_prompt(
    admin_controls: &AdminControls,
    context: AdminAuditContext<'_>,
    prompt_result: Result<Option<String>, io::Error>,
) -> Result<Option<String>, io::Error> {
    let (justification, prompt_error) = match prompt_result {
        Ok(reason) => (reason, None),
        Err(err) => (None, Some(err)),
    };

    maybe_post_admin_audit_events(
        admin_controls,
        AdminAuditContext {
            dangerous_mode_justification: justification.as_deref(),
            ..context
        },
    )
    .await;

    if let Some(err) = prompt_error {
        Err(err)
    } else {
        Ok(justification)
    }
}

pub async fn audit_admin_run_without_prompt(
    admin_controls: &AdminControls,
    context: AdminAuditContext<'_>,
) -> io::Result<()> {
    maybe_post_admin_audit_events(admin_controls, context).await;
    Ok(())
}

async fn post_admin_audit_event(
    client: &Client,
    endpoint: &str,
    audit_log_file: Option<&Path>,
    kind: AdminAuditEventKind,
    details: AdminAuditEventDetails<'_>,
) {
    let username = whoami::username();
    let timestamp_string;
    let payload = {
        timestamp_string = Utc::now().to_rfc3339();
        AdminAuditPayload {
            event: kind.label(),
            sandbox_mode: details.sandbox_mode,
            username,
            dangerously_bypass_requested: details.dangerously_bypass_requested,
            timestamp: &timestamp_string,
            justification: details.justification,
            approval_policy: details.approval_policy,
            cwd: details.cwd,
            command: details.command,
        }
    };

    if let Err(err) = client.post(endpoint).json(&payload).send().await {
        tracing::warn!("Failed to POST admin audit event {}: {err}", kind.label());
    }

    if let Some(path) = audit_log_file
        && let Err(err) = append_admin_audit_log(path, &payload).await
    {
        tracing::warn!(
            "Failed to write admin audit event {} to {}: {err}",
            kind.label(),
            path.display()
        );
    }
}

async fn append_admin_audit_log(path: &Path, payload: &AdminAuditPayload<'_>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(path)
        .await?;

    let mut json_line = serde_json::to_vec(payload).map_err(io::Error::other)?;
    json_line.push(b'\n');
    file.write_all(&json_line).await?;

    Ok(())
}
