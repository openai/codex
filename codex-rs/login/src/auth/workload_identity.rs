use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use codex_config::config_toml::WorkloadIdentityToml;
use codex_workload_identity::FEDERATION_RULE_ID_ENV_VAR;
use codex_workload_identity::IDENTITY_TOKEN_ENV_VAR;
use codex_workload_identity::IDENTITY_TOKEN_FILE_ENV_VAR;
use codex_workload_identity::PRINCIPAL_ID_ENV_VAR;
use codex_workload_identity::TENANT_ID_ENV_VAR;
use codex_workload_identity::WORKSPACE_ID_ENV_VAR;
use codex_workload_identity::WorkloadIdentityAssertionSource;
use codex_workload_identity::WorkloadIdentityConfig;
use codex_workload_identity::WorkloadIdentityExchange;
use codex_workload_identity::WorkloadIdentityTarget;
use codex_workload_identity::WorkloadIdentityToken;

use super::CodexAuth;
use super::ExternalAuth;
use super::ExternalAuthFuture;
use super::ExternalAuthRefreshContext;

pub(super) struct WorkloadIdentityExternalAuth {
    exchange: Result<Arc<WorkloadIdentityExchange>, String>,
}

impl WorkloadIdentityExternalAuth {
    pub(super) fn from_config(config: Option<WorkloadIdentityToml>) -> Option<Self> {
        Self::from_config_and_environment(config, ProcessEnvironment::read())
    }

    fn from_config_and_environment(
        config: Option<WorkloadIdentityToml>,
        environment: ProcessEnvironment,
    ) -> Option<Self> {
        if config.is_none() && !environment.is_configured() {
            return None;
        }
        let exchange = resolve_config(config.unwrap_or_default(), environment)
            .and_then(|config| {
                WorkloadIdentityExchange::new(config).map_err(|error| error.to_string())
            })
            .map(Arc::new);
        Some(Self { exchange })
    }

    fn exchange(&self) -> std::io::Result<&Arc<WorkloadIdentityExchange>> {
        self.exchange
            .as_ref()
            .map_err(|error| std::io::Error::other(error.clone()))
    }

    async fn resolve_auth(&self) -> std::io::Result<CodexAuth> {
        let token = self
            .exchange()?
            .resolve()
            .await
            .map_err(std::io::Error::other)?;
        codex_auth(token)
    }

    async fn refresh_auth(
        &self,
        context: ExternalAuthRefreshContext,
    ) -> std::io::Result<CodexAuth> {
        let token = self
            .exchange()?
            .refresh()
            .await
            .map_err(std::io::Error::other)?;
        if context
            .previous_account_id
            .as_deref()
            .is_some_and(|account_id| account_id != token.chatgpt_account_id)
        {
            return Err(std::io::Error::other(
                "workload identity refresh changed the ChatGPT workspace",
            ));
        }
        codex_auth(token)
    }
}

impl ExternalAuth for WorkloadIdentityExternalAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(self.resolve_auth())
    }

    fn refresh(&self, context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(self.refresh_auth(context))
    }
}

fn codex_auth(token: WorkloadIdentityToken) -> std::io::Result<CodexAuth> {
    CodexAuth::from_external_chatgpt_tokens(
        &token.access_token,
        &token.chatgpt_account_id,
        token.chatgpt_plan_type.as_deref(),
    )
}

#[derive(Default)]
struct ProcessEnvironment {
    federation_rule_id: Option<OsString>,
    identity_token: Option<OsString>,
    identity_token_file: Option<OsString>,
    principal_id: Option<OsString>,
    tenant_id: Option<OsString>,
    workspace_id: Option<OsString>,
}

impl ProcessEnvironment {
    fn read() -> Self {
        Self {
            federation_rule_id: std::env::var_os(FEDERATION_RULE_ID_ENV_VAR),
            identity_token: std::env::var_os(IDENTITY_TOKEN_ENV_VAR),
            identity_token_file: std::env::var_os(IDENTITY_TOKEN_FILE_ENV_VAR),
            principal_id: std::env::var_os(PRINCIPAL_ID_ENV_VAR),
            tenant_id: std::env::var_os(TENANT_ID_ENV_VAR),
            workspace_id: std::env::var_os(WORKSPACE_ID_ENV_VAR),
        }
    }

    fn is_configured(&self) -> bool {
        self.federation_rule_id.is_some()
            || self.identity_token.is_some()
            || self.identity_token_file.is_some()
            || self.principal_id.is_some()
            || self.tenant_id.is_some()
            || self.workspace_id.is_some()
    }
}

fn resolve_config(
    config: WorkloadIdentityToml,
    environment: ProcessEnvironment,
) -> Result<WorkloadIdentityConfig, String> {
    let source = match config.identity_token_file {
        Some(path) => WorkloadIdentityAssertionSource::File(path.into_path_buf()),
        None => match (environment.identity_token, environment.identity_token_file) {
            (Some(token), None) => WorkloadIdentityAssertionSource::Environment(required_unicode(
                Some(token),
                IDENTITY_TOKEN_ENV_VAR,
            )?),
            (None, Some(path)) => WorkloadIdentityAssertionSource::File(PathBuf::from(
                required_unicode(Some(path), IDENTITY_TOKEN_FILE_ENV_VAR)?,
            )),
            (Some(_), Some(_)) => {
                return Err(format!(
                    "set exactly one of {IDENTITY_TOKEN_ENV_VAR} or {IDENTITY_TOKEN_FILE_ENV_VAR}"
                ));
            }
            (None, None) => {
                return Err(format!(
                    "set one of {IDENTITY_TOKEN_ENV_VAR} or {IDENTITY_TOKEN_FILE_ENV_VAR}"
                ));
            }
        },
    };
    let target = WorkloadIdentityTarget {
        federation_rule_id: configured_value(
            config.federation_rule_id,
            environment.federation_rule_id,
            "federation_rule_id",
            FEDERATION_RULE_ID_ENV_VAR,
        )?,
        principal_id: configured_value(
            config.principal_id,
            environment.principal_id,
            "principal_id",
            PRINCIPAL_ID_ENV_VAR,
        )?,
        tenant_id: configured_value(
            config.tenant_id,
            environment.tenant_id,
            "tenant_id",
            TENANT_ID_ENV_VAR,
        )?,
        workspace_id: configured_value(
            config.workspace_id,
            environment.workspace_id,
            "workspace_id",
            WORKSPACE_ID_ENV_VAR,
        )?,
    };
    WorkloadIdentityConfig::new(target, source).map_err(|error| error.to_string())
}

fn configured_value(
    configured: Option<String>,
    environment: Option<OsString>,
    field: &'static str,
    variable: &'static str,
) -> Result<String, String> {
    match configured {
        Some(value) if !value.trim().is_empty() => Ok(value),
        Some(_) => Err(format!("workload identity field {field} must not be empty")),
        None => required_unicode(environment, variable),
    }
}

fn required_unicode(value: Option<OsString>, variable: &'static str) -> Result<String, String> {
    let value = value.ok_or_else(|| format!("workload identity requires {variable}"))?;
    let value = value
        .into_string()
        .map_err(|_| format!("workload identity variable {variable} is invalid"))?;
    if value.trim().is_empty() {
        return Err(format!("workload identity variable {variable} is invalid"));
    }
    Ok(value.trim().to_string())
}

#[cfg(test)]
#[path = "workload_identity_tests.rs"]
mod tests;
