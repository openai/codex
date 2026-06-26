use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::PoisonError;
use std::sync::RwLock;

use codex_apps::CodexApps;
use codex_apps::CodexAppsSnapshot;
use codex_connectors::AppToolPolicyEvaluator;
use codex_connectors::AppToolPolicyInput;
use codex_core::config::Config;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::PromptFragment;
use codex_extension_api::ThreadDataInitializer;
use codex_extension_api::TurnItemContributor;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::TurnItem;

use super::CodexAppsConnectionKey;
use super::CodexAppsMcpExtension;
use super::config::include_apps_instructions;

#[derive(Default)]
pub(super) struct AppsThreadState {
    value: RwLock<AppsThreadValue>,
}

#[derive(Default)]
struct AppsThreadValue {
    revision: u64,
    connection: AppsThreadConnection,
    instructions_available: bool,
}

#[derive(Default)]
enum AppsThreadConnection {
    #[default]
    Empty,
    Discovering {
        key: CodexAppsConnectionKey,
    },
    Ready {
        key: CodexAppsConnectionKey,
        apps: Arc<CodexApps>,
        snapshot: CodexAppsSnapshot,
    },
}

pub(super) enum AppsConnectionPreparation {
    Stale,
    Use(Arc<CodexApps>),
    Initialize { revision: u64 },
}

impl AppsThreadState {
    #[cfg(test)]
    pub(super) fn replace(&self, apps: Option<Arc<CodexApps>>, config: &Config) {
        let snapshot = apps.as_ref().map(|apps| apps.snapshot());
        let instructions_available = instructions_available(snapshot.as_ref(), config);
        let mut value = self.value.write().unwrap_or_else(PoisonError::into_inner);
        *value = AppsThreadValue {
            revision: value.revision.saturating_add(1),
            connection: match (apps, snapshot) {
                (Some(apps), Some(snapshot)) => AppsThreadConnection::Ready {
                    key: CodexAppsConnectionKey {
                        config: codex_apps::CodexAppsConnectConfig::new(
                            config.chatgpt_base_url.clone(),
                            /*product_sku*/ None,
                            config.mcp_oauth_credentials_store_mode,
                            config.auth_keyring_backend_kind(),
                        ),
                        auth_revision: 0,
                    },
                    apps,
                    snapshot,
                },
                _ => AppsThreadConnection::Empty,
            },
            instructions_available,
        };
    }

    pub(super) fn replace_apps_if_revision(
        &self,
        expected_revision: u64,
        connection_key: CodexAppsConnectionKey,
        apps: Arc<CodexApps>,
        snapshot: CodexAppsSnapshot,
        config: &Config,
    ) -> bool {
        let instructions_available = instructions_available(Some(&snapshot), config);
        let mut value = self.value.write().unwrap_or_else(PoisonError::into_inner);
        if value.revision != expected_revision {
            return false;
        }
        *value = AppsThreadValue {
            revision: value.revision.saturating_add(1),
            connection: AppsThreadConnection::Ready {
                key: connection_key,
                apps,
                snapshot,
            },
            instructions_available,
        };
        true
    }

    pub(super) fn prepare_connection_if_revision(
        &self,
        expected_revision: u64,
        connection_key: CodexAppsConnectionKey,
        config: &Config,
    ) -> AppsConnectionPreparation {
        let mut value = self.value.write().unwrap_or_else(PoisonError::into_inner);
        if value.revision != expected_revision {
            return AppsConnectionPreparation::Stale;
        }
        if let AppsThreadConnection::Ready { key, apps, .. } = &value.connection
            && key == &connection_key
        {
            return AppsConnectionPreparation::Use(Arc::clone(apps));
        }
        if let AppsThreadConnection::Discovering { key } = &value.connection
            && key == &connection_key
        {
            return AppsConnectionPreparation::Initialize {
                revision: value.revision,
            };
        }

        *value = AppsThreadValue {
            revision: value.revision.saturating_add(1),
            connection: AppsThreadConnection::Discovering {
                key: connection_key,
            },
            instructions_available: instructions_available(/*snapshot*/ None, config),
        };
        AppsConnectionPreparation::Initialize {
            revision: value.revision,
        }
    }

    pub(super) fn clear_if_revision(&self, expected_revision: u64, config: &Config) -> bool {
        let mut value = self.value.write().unwrap_or_else(PoisonError::into_inner);
        if value.revision != expected_revision {
            return false;
        }
        *value = AppsThreadValue {
            revision: value.revision.saturating_add(1),
            connection: AppsThreadConnection::Empty,
            instructions_available: instructions_available(/*snapshot*/ None, config),
        };
        true
    }

    pub(super) fn revision(&self) -> u64 {
        self.value
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .revision
    }

    pub(super) fn apps_for_key(
        &self,
        connection_key: &CodexAppsConnectionKey,
    ) -> Option<Arc<CodexApps>> {
        let value = self.value.read().unwrap_or_else(PoisonError::into_inner);
        match &value.connection {
            AppsThreadConnection::Ready { key, apps, .. } if key == connection_key => {
                Some(Arc::clone(apps))
            }
            _ => None,
        }
    }

    pub(super) fn snapshot(&self) -> Option<CodexAppsSnapshot> {
        let value = self.value.read().unwrap_or_else(PoisonError::into_inner);
        match &value.connection {
            AppsThreadConnection::Ready { snapshot, .. } => Some(snapshot.clone()),
            _ => None,
        }
    }

    fn instructions_available(&self) -> bool {
        self.value
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .instructions_available
    }
}

fn instructions_available(snapshot: Option<&CodexAppsSnapshot>, config: &Config) -> bool {
    include_apps_instructions(config)
        && snapshot.is_some_and(|snapshot| {
            let evaluator = AppToolPolicyEvaluator::new(&config.config_layer_stack);
            snapshot.tools().any(|(_, _, metadata)| {
                evaluator
                    .policy(AppToolPolicyInput {
                        connector_id: Some(metadata.connector_id()),
                        tool_name: metadata.upstream_tool_name(),
                        tool_title: metadata.tool_title(),
                        destructive_hint: metadata.destructive_hint(),
                        open_world_hint: metadata.open_world_hint(),
                    })
                    .enabled
            })
        })
}

#[derive(Clone)]
struct AppsTurnItemPresentation {
    connector_id: String,
    connector_name: String,
    link_id: Option<String>,
    mcp_app_resource_uri: Option<String>,
    template_id: Option<String>,
    action_name: Option<String>,
}

#[derive(Default)]
struct AppsTurnItemState {
    by_call: StdMutex<HashMap<AppsTurnItemKey, AppsTurnItemPresentation>>,
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct AppsTurnItemKey {
    id: String,
    server: String,
    tool: String,
}

impl ContextContributor for CodexAppsMcpExtension {
    fn contribute_thread_context<'a>(
        &'a self,
        _session_store: &'a codex_extension_api::ExtensionData,
        thread_store: &'a codex_extension_api::ExtensionData,
    ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
        Box::pin(async move {
            if !thread_store
                .get::<AppsThreadState>()
                .is_some_and(|state| state.instructions_available())
            {
                return Vec::new();
            }
            vec![PromptFragment::developer_capability(format!(
                "{}\n## Apps (Connectors)\nApps (Connectors) can be explicitly triggered in user messages in the format `[$app-name](app://{{connector_id}})`. Apps can also be implicitly triggered as long as the context suggests usage of available apps.\nEach installed app is exposed as an ordinary MCP server with its own namespace.\nAn installed app's MCP tools are either provided to you already, or can be lazy-loaded through the `tool_search` tool.\nDo not additionally call `list_mcp_resources` or `list_mcp_resource_templates` for apps.\n{}",
                codex_protocol::protocol::APPS_INSTRUCTIONS_OPEN_TAG,
                codex_protocol::protocol::APPS_INSTRUCTIONS_CLOSE_TAG,
            ))]
        })
    }
}

impl ThreadDataInitializer for CodexAppsMcpExtension {
    fn initialize(&self, thread_data: &mut codex_extension_api::ExtensionDataInit) {
        if thread_data.get::<AppsThreadState>().is_none() {
            thread_data.insert(AppsThreadState::default());
        }
    }
}

impl TurnItemContributor for CodexAppsMcpExtension {
    fn applies_to(&self, item: &TurnItem) -> bool {
        matches!(item, TurnItem::McpToolCall(_))
    }

    fn contribute<'a>(
        &'a self,
        thread_store: &'a codex_extension_api::ExtensionData,
        turn_store: &'a codex_extension_api::ExtensionData,
        item: &'a mut TurnItem,
    ) -> ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let TurnItem::McpToolCall(item) = item else {
                return Ok(());
            };
            let Some(state) = thread_store.get::<AppsThreadState>() else {
                return Ok(());
            };
            let call_state = turn_store.get_or_init(AppsTurnItemState::default);
            let key = AppsTurnItemKey {
                id: item.id.clone(),
                server: item.server.clone(),
                tool: item.tool.clone(),
            };
            let mut by_call = call_state
                .by_call
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let cached = if item.status == McpToolCallStatus::InProgress {
                None
            } else {
                by_call.remove(&key)
            };
            let presentation = cached.or_else(|| {
                let snapshot = state.snapshot()?;
                let metadata = snapshot.tool_metadata(&key.server, &item.tool)?;
                Some(AppsTurnItemPresentation {
                    connector_id: metadata.connector_id().to_string(),
                    connector_name: metadata.connector_name().to_string(),
                    link_id: metadata.link_id().map(str::to_string),
                    mcp_app_resource_uri: metadata.mcp_app_resource_uri().map(str::to_string),
                    template_id: metadata.template_id().map(str::to_string),
                    action_name: metadata.action_name().map(str::to_string),
                })
            });
            let Some(presentation) = presentation else {
                return Ok(());
            };
            if item.status == McpToolCallStatus::InProgress {
                by_call.insert(key, presentation.clone());
            }
            drop(by_call);
            if item.status != McpToolCallStatus::InProgress && item.duration.is_some() {
                super::analytics::remember_app_tool_usage(
                    turn_store,
                    &item.id,
                    &presentation.connector_id,
                    &presentation.connector_name,
                );
            }
            item.connector_id = Some(presentation.connector_id);
            item.link_id = presentation.link_id;
            item.app_name = Some(presentation.connector_name);
            item.template_id = presentation.template_id;
            item.action_name = presentation.action_name;
            if item.mcp_app_resource_uri.is_none() {
                item.mcp_app_resource_uri = presentation.mcp_app_resource_uri;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
