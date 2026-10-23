//! First-thread overrides come from invocation inputs and deliberate product choices.
//! Explicit profile launches retain their existing resolved-config path until server profile
//! selection is available for connected launches.

use crate::legacy_core::config::ConfigOverrides;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ThreadStartParams;
use codex_config::LoaderOverrides;
use codex_protocol::protocol::AskForApproval as CoreAskForApproval;
use std::collections::HashMap;

#[derive(Clone, Default)]
pub(crate) struct StartupLaunchChoices {
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<serde_json::Value>,
    provider: Option<String>,
    selected_profile: bool,
    pub(super) approval_policy: StartupApprovalPolicy,
    pub(super) approvals_reviewer: StartupApprovalsReviewer,
}

#[derive(Clone, Default)]
pub(super) enum StartupApprovalPolicy {
    #[default]
    ResolvedConfig,
    Invocation(Option<AskForApproval>),
    ConfigOverride(HashMap<String, serde_json::Value>),
}

#[derive(Clone, Copy, Default)]
pub(super) enum StartupApprovalsReviewer {
    #[default]
    Config,
    Invocation(Option<ApprovalsReviewer>),
}

impl StartupLaunchChoices {
    pub(crate) fn from_launch(
        cli_overrides: &[(String, toml::Value)],
        harness_overrides: &ConfigOverrides,
        loader_overrides: &LoaderOverrides,
    ) -> Self {
        if loader_overrides.user_config_profile.is_some() {
            return Self {
                provider: harness_overrides.model_provider.clone(),
                selected_profile: true,
                ..Self::default()
            };
        }
        let value = |key| {
            cli_overrides
                .iter()
                .rev()
                .find(|(path, _)| path == key)
                .map(|(_, value)| value)
        };
        let approval_policy = if let Some(policy) = harness_overrides.approval_policy {
            StartupApprovalPolicy::Invocation(Some(policy.into()))
        } else if let Some(value) =
            codex_config::build_cli_overrides_layer(cli_overrides).get("approval_policy")
        {
            if value.is_str()
                && let Ok(policy) = value.clone().try_into::<CoreAskForApproval>()
            {
                StartupApprovalPolicy::Invocation(Some(policy.into()))
            } else {
                // Preserve each original assignment boundary: parent assignments replace,
                // while dotted assignments merge with the server's CLI layer.
                let overrides = cli_overrides
                    .iter()
                    .filter_map(|(path, _)| {
                        let mut keys = path.split('.');
                        if keys.next()? != "approval_policy"
                            || cli_overrides.iter().any(|(parent, _)| {
                                path.strip_prefix(parent)
                                    .is_some_and(|suffix| suffix.starts_with('.'))
                            })
                        {
                            return None;
                        }
                        let value = keys.try_fold(value, |value, key| value.get(key))?;
                        Some((path.clone(), serde_json::json!(value)))
                    })
                    .collect();
                StartupApprovalPolicy::ConfigOverride(overrides)
            }
        } else {
            StartupApprovalPolicy::Invocation(None)
        };
        Self {
            model: harness_overrides.model.clone().or_else(|| {
                value("model")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
            }),
            effort: value("model_reasoning_effort")
                .and_then(|value| serde_json::to_value(value).ok()),
            provider: harness_overrides.model_provider.clone().or_else(|| {
                value("model_provider")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
            }),
            selected_profile: false,
            approval_policy,
            approvals_reviewer: StartupApprovalsReviewer::Invocation(
                harness_overrides
                    .approvals_reviewer
                    .or_else(|| value("approvals_reviewer")?.clone().try_into().ok())
                    .map(Into::into),
            ),
        }
    }

    pub(crate) fn apply_projectless_approval_policy(
        &mut self,
        policy: Option<CoreAskForApproval>,
        allowed: Option<&[AskForApproval]>,
    ) {
        if let StartupApprovalPolicy::Invocation(None) = self.approval_policy
            && let Some(policy) = policy.map(AskForApproval::from)
            && allowed.is_none_or(|allowed| allowed.contains(&policy))
        {
            self.approval_policy = StartupApprovalPolicy::Invocation(Some(policy));
        }
    }

    pub(super) fn configure(self, params: &mut ThreadStartParams) {
        if self.selected_profile {
            params.model_provider = self.provider.or(params.model_provider.take());
            return;
        }
        if let StartupApprovalPolicy::ConfigOverride(overrides) = self.approval_policy {
            params.config.get_or_insert_default().extend(overrides);
        }
        params.model = self.model;
        params.model_provider = self.provider;
        if let Some(overrides) = params.config.as_mut() {
            overrides.remove("model_reasoning_effort");
        }
        if let Some(effort) = self.effort {
            params
                .config
                .get_or_insert_default()
                .insert("model_reasoning_effort".into(), effort);
        }
    }
}
