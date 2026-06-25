use std::sync::Arc;
use std::sync::atomic::Ordering;

use codex_apps::CODEX_APPS_RESOURCE_MCP_SERVER_NAME;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributionMode;
use codex_extension_api::McpServerContributor;

use super::CodexAppsMcpExtension;
use super::config::apps_mcp_eligible;
use super::config::current_auth_revision;
use super::policy::apply_apps_server_policy;
use super::presentation::AppsConnectionPreparation;
use super::presentation::AppsThreadState;
use crate::executor_plugin::selected_plugin_connector_snapshot;

const CODEX_APPS_EXTENSION_ID: &str = "codex_apps";

impl McpServerContributor<codex_core::config::Config> for CodexAppsMcpExtension {
    fn id(&self) -> &'static str {
        CODEX_APPS_EXTENSION_ID
    }

    fn revision(&self) -> u64 {
        self.connection
            .publication_revision
            .load(Ordering::Acquire)
            .saturating_add(current_auth_revision(&self.connection.auth_manager))
    }

    fn contribute<'a>(
        &'a self,
        context: McpServerContributionContext<'a, codex_core::config::Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            let config = context.config();
            let thread_state = context
                .thread_init()
                .and_then(codex_extension_api::ExtensionDataInit::get::<AppsThreadState>);
            let thread_state_revision = thread_state.as_ref().map(|state| state.revision());
            if !apps_mcp_eligible(config) {
                // Guardian shares the process extension registry and disables Apps. Do not touch
                // the process-wide Apps connection from an ineligible child session.
                if let Some((state, revision)) = thread_state.as_ref().zip(thread_state_revision) {
                    state.clear_if_revision(revision, config);
                }
                return Vec::new();
            }

            let Some(connection_key) = self.connection.connection_key(config).await else {
                if let Some((state, revision)) = thread_state.as_ref().zip(thread_state_revision) {
                    state.clear_if_revision(revision, config);
                }
                return Vec::new();
            };
            let apps = self
                .connection
                .current_apps_for_key(&connection_key)
                .or_else(|| {
                    thread_state
                        .as_ref()
                        .and_then(|state| state.apps_for_key(&connection_key))
                });
            let mode = context.mode();
            let (connection_key, apps, state_revision) = match apps {
                Some(apps) => (connection_key, apps, thread_state_revision),
                None => match mode {
                    McpServerContributionMode::Current => return Vec::new(),
                    McpServerContributionMode::Discover => {
                        let Some((state, revision)) =
                            thread_state.as_ref().zip(thread_state_revision)
                        else {
                            self.initialize_in_background(
                                config.clone(),
                                connection_key,
                                /*thread_state*/ None,
                            );
                            return Vec::new();
                        };
                        match state.prepare_connection_if_revision(
                            revision,
                            connection_key.clone(),
                            config,
                        ) {
                            AppsConnectionPreparation::Stale => return Vec::new(),
                            AppsConnectionPreparation::Use(apps) => {
                                (connection_key, apps, Some(revision))
                            }
                            AppsConnectionPreparation::Initialize { revision } => {
                                self.initialize_in_background(
                                    config.clone(),
                                    connection_key,
                                    Some((Arc::clone(state), revision)),
                                );
                                return Vec::new();
                            }
                        }
                    }
                },
            };
            match mode {
                McpServerContributionMode::Discover => {
                    self.refresh_in_background(connection_key.clone(), Arc::clone(&apps));
                }
                McpServerContributionMode::Current => {}
            }
            let snapshot = apps.snapshot();
            if let Some((state, revision)) = thread_state.as_ref().zip(state_revision)
                && !state.replace_apps_if_revision(
                    revision,
                    connection_key,
                    Arc::clone(&apps),
                    snapshot.clone(),
                    config,
                )
            {
                return Vec::new();
            }
            let selected_plugin_connectors = selected_plugin_connector_snapshot(context);
            let plugin_connectors = self
                .plugin_connector_snapshot(config)
                .await
                .merged_with(&selected_plugin_connectors);
            let mut servers = apply_apps_server_policy(
                config,
                &snapshot,
                &plugin_connectors,
                snapshot
                    .effective_mcp_servers()
                    .into_iter()
                    .collect::<Vec<_>>(),
            );
            servers.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut contributions = vec![McpServerContribution::SetEffective {
                name: CODEX_APPS_RESOURCE_MCP_SERVER_NAME.to_string(),
                server: Box::new(snapshot.resource_mcp_server()),
            }];
            contributions.extend(servers.into_iter().map(|(name, server)| {
                McpServerContribution::SetEffective {
                    name,
                    server: Box::new(server),
                }
            }));
            contributions
        })
    }

    fn refresh<'a>(
        &'a self,
        context: McpServerContributionContext<'a, codex_core::config::Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            match context.mode() {
                McpServerContributionMode::Current => return,
                McpServerContributionMode::Discover => {}
            }
            let config = context.config();
            let thread_state = context
                .thread_init()
                .and_then(codex_extension_api::ExtensionDataInit::get::<AppsThreadState>);
            let thread_state_revision = thread_state.as_ref().map(|state| state.revision());
            if !apps_mcp_eligible(config) {
                if let Some((state, revision)) = thread_state.as_ref().zip(thread_state_revision) {
                    state.clear_if_revision(revision, config);
                }
                return;
            }
            match self
                .connection
                .apps_for_config(config, /*refresh*/ true)
                .await
            {
                Ok(Some((connection_key, apps))) => {
                    if let Some((state, revision)) =
                        thread_state.as_ref().zip(thread_state_revision)
                    {
                        let snapshot = apps.snapshot();
                        state.replace_apps_if_revision(
                            revision,
                            connection_key,
                            apps,
                            snapshot,
                            config,
                        );
                    }
                }
                Ok(None) => {
                    if let Some((state, revision)) =
                        thread_state.as_ref().zip(thread_state_revision)
                    {
                        state.clear_if_revision(revision, config);
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to refresh Codex Apps MCP");
                }
            }
        })
    }
}

#[cfg(test)]
#[path = "contributor_tests.rs"]
mod tests;
