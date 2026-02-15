use crate::app_event::AppEvent;
#[cfg(target_os = "windows")]
use crate::app_event::WindowsSandboxEnableMode;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::history_cell;
use codex_core::config::Config;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::WindowsSandboxLevel;

/// A simple preset pairing an approval policy with a sandbox policy.
#[derive(Debug, Clone)]
pub struct PermissionsPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Display label shown in UIs.
    pub label: &'static str,
    /// Short human description shown next to the label in UIs.
    pub description: &'static str,
    /// Approval policy to apply.
    pub approval: AskForApproval,
    /// Sandbox policy to apply.
    pub sandbox: SandboxPolicy,
}

/// Built-in list of approval presets that pair approval and sandbox policy.
pub fn builtin_permissions_presets() -> Vec<PermissionsPreset> {
    vec![
        PermissionsPreset {
            id: "read-only",
            label: "Read Only",
            description: "Codex can read files in the current workspace. Approval is required to edit files or access the internet.",
            approval: AskForApproval::OnRequest,
            sandbox: SandboxPolicy::new_read_only_policy(),
        },
        PermissionsPreset {
            id: "auto",
            label: "Default",
            description: "Codex can read and edit files in the current workspace, and run commands. Approval is required to access the internet or edit other files. (Identical to Agent mode)",
            approval: AskForApproval::OnRequest,
            sandbox: SandboxPolicy::new_workspace_write_policy(),
        },
        PermissionsPreset {
            id: "full-access",
            label: "Full Access",
            description: "Codex can edit files outside this workspace and access the internet without asking for approval. Exercise caution when using.",
            approval: AskForApproval::Never,
            sandbox: SandboxPolicy::DangerFullAccess,
        },
    ]
}

pub(crate) fn visible_permissions_options(config: &Config) -> Vec<SelectionItem> {
    builtin_permissions_presets()
        .into_iter()
        .filter(|preset| preset.is_visible(config))
        .map(|preset| preset.to_selection_item(config))
        .collect()
}

impl PermissionsPreset {
    pub(crate) fn is_visible(&self, config: &Config) -> bool {
        match self.id {
            "read-only" => cfg!(target_os = "windows"),
            "auto" => {
                !cfg!(target_os = "windows")
                    || codex_core::windows_sandbox::windows_sandbox_level_from_config(config)
                        == WindowsSandboxLevel::Disabled
            }
            "full-access" => true,
            _ => false,
        }
    }

    pub(crate) fn to_selection_item(&self, config: &Config) -> SelectionItem {
        let name = if self.id == "auto" && windows_degraded_sandbox_enabled(config) {
            "Default (non-admin sandbox)".to_string()
        } else {
            self.label.to_string()
        };

        SelectionItem {
            name,
            description: Some(self.description.to_string()),
            is_current: self.is_current(config),
            actions: self.actions(config),
            dismiss_on_select: true,
            disabled_reason: self.disabled_reason(config),
            ..Default::default()
        }
    }

    fn is_current(&self, config: &Config) -> bool {
        self.approval == config.permissions.approval_policy.value()
            && self.sandbox == *config.permissions.sandbox_policy.get()
    }

    fn disabled_reason(&self, config: &Config) -> Option<String> {
        let disabled_sandbox_reason = match config.permissions.sandbox_policy.can_set(&self.sandbox)
        {
            Ok(()) => None,
            Err(err) => Some(err.to_string()),
        };
        if disabled_sandbox_reason.is_some() {
            return disabled_sandbox_reason;
        }

        let disabled_approval_reason =
            match config.permissions.approval_policy.can_set(&self.approval) {
                Ok(()) => None,
                Err(err) => Some(err.to_string()),
            };
        if disabled_approval_reason.is_some() {
            return disabled_approval_reason;
        }

        None
    }

    pub(crate) fn actions(&self, config: &Config) -> Vec<SelectionAction> {
        let requires_full_access_confirmation =
            self.id == "full-access" && !config.notices.hide_full_access_warning.unwrap_or(false);
        if requires_full_access_confirmation {
            let preset = self.clone();
            return vec![Box::new(move |tx: &AppEventSender| {
                tx.send(AppEvent::OpenFullAccessConfirmation {
                    preset: preset.clone(),
                });
            })];
        }

        #[cfg(target_os = "windows")]
        {
            if let Some(actions) = windows_permissions_actions(self, config) {
                return actions;
            }
        }

        let approval = self.approval;
        let sandbox = self.sandbox.clone();
        let label = self.label.to_string();
        vec![Box::new(move |tx: &AppEventSender| {
            let sandbox_clone = sandbox.clone();
            tx.send(AppEvent::CodexOp(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: Some(approval),
                sandbox_policy: Some(sandbox_clone.clone()),
                windows_sandbox_level: None,
                model: None,
                effort: None,
                summary: None,
                collaboration_mode: None,
                personality: None,
            }));
            tx.send(AppEvent::UpdateAskForApprovalPolicy(approval));
            tx.send(AppEvent::UpdateSandboxPolicy(sandbox_clone));
            tx.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_info_event(format!("Permissions updated to {label}"), None),
            )));
        })]
    }
}

/// Handle windows-specific actions for auto preset. Returns Some when it should take precedence over the approval preset actions.
#[cfg(target_os = "windows")]
fn windows_permissions_actions(
    preset: &PermissionsPreset,
    config: &Config,
) -> Option<Vec<SelectionAction>> {
    if preset.id != "auto" {
        return None;
    }

    if codex_core::windows_sandbox::windows_sandbox_level_from_config(config)
        == WindowsSandboxLevel::Disabled
    {
        let preset_clone = preset.clone();
        if codex_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
            && codex_core::windows_sandbox::sandbox_setup_is_complete(config.codex_home.as_path())
        {
            Some(vec![Box::new(move |tx| {
                tx.send(AppEvent::EnableWindowsSandboxForAgentMode {
                    preset: preset_clone.clone(),
                    mode: WindowsSandboxEnableMode::Elevated,
                });
            })])
        } else {
            Some(vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenWindowsSandboxEnablePrompt {
                    preset: preset_clone.clone(),
                });
            })])
        }
    } else if let Some((sample_paths, extra_count, failed_scan)) =
        world_writable_warning_details(config)
    {
        let preset_clone = preset.clone();
        Some(vec![Box::new(move |tx| {
            tx.send(AppEvent::OpenWorldWritableWarningConfirmation {
                preset: Some(preset_clone.clone()),
                sample_paths: sample_paths.clone(),
                extra_count,
                failed_scan,
            });
        })])
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_degraded_sandbox_enabled(config: &Config) -> bool {
    let windows_sandbox_level =
        codex_core::windows_sandbox::windows_sandbox_level_from_config(config);
    matches!(windows_sandbox_level, WindowsSandboxLevel::RestrictedToken);
}

#[cfg(target_os = "windows")]
fn world_writable_warning_details(config: &Config) -> Option<(Vec<String>, usize, bool)> {
    if config.notices.hide_world_writable_warning.unwrap_or(false) {
        return None;
    }
    let cwd = config.cwd.clone();
    let env_map: std::collections::HashMap<String, String> = std::env::vars().collect();
    match codex_windows_sandbox::apply_world_writable_scan_and_denies(
        config.codex_home.as_path(),
        cwd.as_path(),
        &env_map,
        config.permissions.sandbox_policy.get(),
        Some(config.codex_home.as_path()),
    ) {
        Ok(_) => None,
        Err(_) => Some((Vec::new(), 0, true)),
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn windows_degraded_sandbox_enabled(_config: &Config) -> bool {
    false
}
