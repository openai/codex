//! Diffs the one-level Responses Lite catalog: namespaces of callable tools and built-ins.
//! Missing state starts a fresh catalog; this section does not migrate legacy history.
//! One developer notice precedes each incremental namespace batch; catalog hashes are unchanged.
//! Whole namespace removals subsume their members in the removal notice.
//! Metrics count rendered tool deltas and measure replacement description bytes.

use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::ContextualUserFragment;
use crate::context::DeveloperInstructions;
use codex_code_mode::ExecDefinition;
use codex_code_mode::PUBLIC_TOOL_NAME;
use codex_extension_api::ExtensionMetrics;
use codex_otel::EXEC_TOOL_ENTRY_UPDATES_METRIC;
use codex_otel::EXEC_TOOL_UPDATE_BYTES_METRIC;
use codex_otel::TOOL_INCREMENTAL_UPDATES_METRIC;
use codex_otel::TOOL_SPEC_DESCRIPTION_BYTES_METRIC;
use codex_otel::TOOL_SPEC_DESCRIPTION_BYTES_METRIC_BUCKETS;
use codex_prompts::ResolvedIncrementalToolMessages;
use codex_protocol::DEFAULT_FUNCTION_NAMESPACE;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::input_text;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;

const ACTION_ADDED: &str = "added";
const ACTION_REMOVED: &str = "removed";
const ACTION_SCHEMA_CHANGED: &str = "schema_changed";

/// The exact serialized Responses Lite declarations visible in one captured step.
pub(crate) struct TopLevelToolsState {
    messages: ResolvedIncrementalToolMessages,
    definitions: Vec<Value>,
    hashes: BTreeMap<String, WorldStateHash>,
    metrics: Option<Arc<dyn ExtensionMetrics>>,
    /// Whether this captured step uses granular exec catalog updates.
    incremental_exec_tools: bool,
    /// Diffable entries for the current CodeModeOnly catalog; absent in other tool modes.
    exec_definitions: Option<Arc<Vec<ExecDefinition>>>,
}

/// Ordinary catalogs keep their landed flat format; exec mode adds its own comparison state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum TopLevelToolsSnapshot {
    WithExec {
        tools: BTreeMap<String, WorldStateHash>,
        /// Stable entry keys and definition hashes, retained while exec is unavailable.
        exec: BTreeMap<String, WorldStateHash>,
    },
    ToolsOnly(BTreeMap<String, WorldStateHash>),
}

impl TopLevelToolsState {
    #[expect(
        clippy::expect_used,
        reason = "the Responses Lite serializer emits objects with array-valued namespace members"
    )]
    pub(crate) fn new(
        messages: ResolvedIncrementalToolMessages,
        definitions: Vec<Value>,
        metrics: Option<Arc<dyn ExtensionMetrics>>,
    ) -> Result<Self> {
        let mut hashes = BTreeMap::new();
        let mut insert_hash = |name: String, definition: &Value| -> Result<()> {
            let hash = WorldStateHash::from_json(definition);
            if hashes.insert(name.clone(), hash).is_some() {
                return Err(CodexErrorDetails::ToolCollision(name).into());
            }
            Ok(())
        };
        for definition in &definitions {
            let name = definition_name(definition);
            let mut header = definition.clone();
            if let Some(tools) = header
                .as_object_mut()
                .expect("serialized tool object")
                .remove("tools")
            {
                for tool in tools.as_array().expect("serialized namespace members") {
                    insert_hash(format!("{name}.{}", definition_name(tool)), tool)?;
                }
            }
            insert_hash(name.to_string(), &header)?;
        }
        Ok(Self {
            messages,
            definitions,
            hashes,
            metrics,
            incremental_exec_tools: false,
            exec_definitions: None,
        })
    }

    /// Track granular exec updates as part of this tool catalog.
    pub(crate) fn with_incremental_exec_tools(
        mut self,
        exec_definitions: Option<Arc<Vec<ExecDefinition>>>,
    ) -> Self {
        self.incremental_exec_tools = true;
        self.exec_definitions = exec_definitions;
        self
    }
}

// Input comes from create_tools_json_for_responses_lite: namespaces and their members have
// names; built-ins are identified by type. Namespace members cannot contain namespaces.
#[expect(
    clippy::expect_used,
    reason = "the Responses Lite serializer gives every declaration a name or type"
)]
fn definition_name(definition: &Value) -> &str {
    definition["name"]
        .as_str()
        .unwrap_or_else(|| definition["type"].as_str().expect("serialized tool type"))
}

fn record_changed_tool_metrics(
    name: &str,
    definition: &Value,
    previous: Option<&BTreeMap<String, WorldStateHash>>,
    metrics: Option<&dyn ExtensionMetrics>,
) {
    const ACTION_TAG: &str = "action";
    const TOOL_TAG: &str = "tool";

    let (Some(previous), Some(metrics)) = (previous, metrics) else {
        return;
    };
    let action = if previous.contains_key(name) {
        ACTION_SCHEMA_CHANGED
    } else {
        ACTION_ADDED
    };
    metrics.counter(
        TOOL_INCREMENTAL_UPDATES_METRIC,
        /*inc*/ 1,
        &[(ACTION_TAG, action), (TOOL_TAG, name)],
    );
    if action == ACTION_SCHEMA_CHANGED
        && let Some(description) = definition["description"].as_str()
    {
        metrics.histogram_with_boundaries(
            TOOL_SPEC_DESCRIPTION_BYTES_METRIC,
            i64::try_from(description.len()).unwrap_or(i64::MAX),
            TOOL_SPEC_DESCRIPTION_BYTES_METRIC_BUCKETS.as_slice(),
            &[(TOOL_TAG, name)],
        );
    }
}

impl WorldStateSection for TopLevelToolsState {
    const ID: &'static str = "top_level_tools";
    type Snapshot = TopLevelToolsSnapshot;

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        let (previous, previous_exec) = match previous {
            PreviousSectionState::Known(TopLevelToolsSnapshot::WithExec { tools, exec }) => {
                (Some(tools), Some(exec))
            }
            PreviousSectionState::Known(TopLevelToolsSnapshot::ToolsOnly(tools)) => {
                (Some(tools), None)
            }
            PreviousSectionState::Absent | PreviousSectionState::Unknown => (None, None),
        };
        // Only an advertised CodeModeOnly exec declaration gets granular updates.
        let exec_catalog = self.exec_definitions.as_ref().and_then(|entries| {
            self.definitions.iter().find_map(|namespace| {
                if namespace["type"] != "namespace"
                    || namespace["name"] != DEFAULT_FUNCTION_NAMESPACE
                {
                    return None;
                }
                let exec = namespace["tools"]
                    .as_array()?
                    .iter()
                    .find(|tool| tool["type"] == "custom" && tool["name"] == PUBLIC_TOOL_NAME)?;
                Some((namespace, exec, entries.as_slice()))
            })
        });
        let manages_exec = exec_catalog.is_some();
        let mut updates = self.render_declaration_updates(previous, manages_exec);
        self.append_removed_tools(previous, &mut updates);
        let snapshot = if self.incremental_exec_tools {
            let exec =
                self.append_exec_updates(previous, previous_exec, exec_catalog, &mut updates);
            TopLevelToolsSnapshot::WithExec {
                tools: self.hashes.clone(),
                exec,
            }
        } else {
            TopLevelToolsSnapshot::ToolsOnly(self.hashes.clone())
        };
        // Persist empty catalogs as known comparison baselines.
        (Some(snapshot), updates)
    }
}

impl TopLevelToolsState {
    /// Render added or changed declarations and namespace instruction updates.
    fn render_declaration_updates(
        &self,
        previous: Option<&BTreeMap<String, WorldStateHash>>,
        manages_exec: bool,
    ) -> Vec<WorldStateUpdate> {
        let changed =
            |name: &str| previous.and_then(|previous| previous.get(name)) != self.hashes.get(name);
        let mut namespace_updates = Vec::new();
        let mut tools = Vec::new();
        // Tool declarations: collect changed standalone tools and namespace members.
        for definition in &self.definitions {
            let name = definition_name(definition);
            let Some(members) = definition["tools"].as_array() else {
                if changed(name) {
                    record_changed_tool_metrics(
                        name,
                        definition,
                        previous,
                        self.metrics.as_deref(),
                    );
                    tools.push(definition.clone());
                }
                continue;
            };
            let mut changed_members = Vec::new();
            for tool in members {
                let tool_name = format!("{name}.{}", definition_name(tool));
                if manages_exec && previous.is_some() && tool_name == "functions.exec" {
                    continue;
                }
                if changed(&tool_name) {
                    record_changed_tool_metrics(
                        &tool_name,
                        tool,
                        previous,
                        self.metrics.as_deref(),
                    );
                    changed_members.push(tool.clone());
                }
            }
            if !changed_members.is_empty() {
                let mut namespace = definition.clone();
                namespace["tools"] = Value::Array(changed_members);
                tools.push(namespace);
            } else if changed(name) && (previous.is_some() || !manages_exec) {
                // Namespace instruction updates use text because declarations require a member.
                let instructions = definition["description"].as_str().unwrap_or_default();
                let text = if instructions.is_empty() {
                    self.messages
                        .namespace_instructions_cleared
                        .replace("{name}", name)
                } else {
                    let mut text = self
                        .messages
                        .namespace_instructions_prefix
                        .replace("{name}", name);
                    text.push_str(instructions);
                    text
                };
                if !text.is_empty() {
                    namespace_updates.push(WorldStateUpdate::fragment(
                        DeveloperInstructions::new(text).with_metadata(ContentItemMetadata::tool(
                            Some(name.to_string().into()),
                        )),
                    ));
                }
            }
        }
        let mut updates = Vec::new();
        if !tools.is_empty() {
            // Incremental tool updates: introduce an appended batch of namespace declarations.
            if previous.is_some()
                && tools.iter().any(|tool| tool["type"] == "namespace")
                && !self.messages.tool_update_hint.is_empty()
            {
                updates.push(WorldStateUpdate::prefix_item(ContextualUserFragment::into(
                    IncrementalToolsUpdate(self.messages.tool_update_hint.clone()),
                )));
            }
            updates.push(WorldStateUpdate::prefix_item(
                ResponseItem::AdditionalTools {
                    id: None,
                    role: "developer".to_string(),
                    tools: Arc::from(tools),
                },
            ));
        }
        updates.extend(namespace_updates);
        updates
    }

    /// Append a removal notice, listing removed namespaces instead of their individual members.
    fn append_removed_tools(
        &self,
        previous: Option<&BTreeMap<String, WorldStateHash>>,
        updates: &mut Vec<WorldStateUpdate>,
    ) {
        if let Some(previous) = previous {
            // Removal notices: withdraw namespaces and tools absent from the current catalog.
            // Namespace and member names cannot contain dots. A qualified key identifies
            // its old namespace without changing the persisted header/member hash format.
            let namespaces = previous
                .keys()
                .filter_map(|key| key.split_once('.').map(|(namespace, _)| namespace))
                .filter(|namespace| {
                    previous.contains_key(*namespace) && !self.hashes.contains_key(*namespace)
                })
                .collect::<BTreeSet<_>>();
            let tools = previous
                .keys()
                .filter(|key| !self.hashes.contains_key(*key))
                .filter(|key| !namespaces.contains(key.as_str()))
                .inspect(|name| {
                    // Count members even when the notice collapses their removed namespace.
                    if let Some(metrics) = self.metrics.as_deref() {
                        metrics.counter(
                            TOOL_INCREMENTAL_UPDATES_METRIC,
                            /*inc*/ 1,
                            &[("action", ACTION_REMOVED), ("tool", name.as_str())],
                        );
                    }
                })
                .filter(|key| {
                    !key.split_once('.')
                        .is_some_and(|(namespace, _)| namespaces.contains(namespace))
                })
                .cloned()
                .collect::<Vec<_>>();
            if !namespaces.is_empty() || !tools.is_empty() {
                updates.push(
                    WorldStateUpdate::fragment(RemovedTools {
                        messages: self.messages.clone(),
                        namespaces: namespaces.into_iter().map(str::to_string).collect(),
                        tools,
                    })
                    .standalone(),
                );
            }
        }
    }

    /// Append exec catalog updates and return the matching comparison snapshot.
    fn append_exec_updates(
        &self,
        previous: Option<&BTreeMap<String, WorldStateHash>>,
        previous_exec: Option<&BTreeMap<String, WorldStateHash>>,
        exec_catalog: Option<(&Value, &Value, &[ExecDefinition])>,
        updates: &mut Vec<WorldStateUpdate>,
    ) -> BTreeMap<String, WorldStateHash> {
        // Retain entry hashes while functions.exec is not advertised.
        // When it returns, send the full current catalog and withdraw removed entries.
        let mut exec = BTreeMap::new();
        if let Some((namespace, declaration, entries)) = exec_catalog {
            exec = entries
                .iter()
                .map(|entry| {
                    (
                        entry.key.clone(),
                        WorldStateHash::from_json(&Value::String(entry.description.clone())),
                    )
                })
                .collect();
            if previous_exec.is_some()
                && previous.is_some_and(|tools| tools.contains_key("functions.exec"))
            {
                // Incremental tool updates: append entries; the exec declaration stays frozen.
                let changed = entries
                    .iter()
                    .filter(|entry| {
                        previous_exec.and_then(|previous| previous.get(&entry.key))
                            != exec.get(&entry.key)
                    })
                    .collect::<Vec<_>>();
                if !changed.is_empty() {
                    let update =
                        render_exec_update(&changed, &self.messages.exec_tools_update_hint);
                    if let Some(metrics) = self.metrics.as_deref() {
                        let added = changed
                            .iter()
                            .filter(|entry| {
                                !previous_exec
                                    .is_some_and(|previous| previous.contains_key(&entry.key))
                            })
                            .count();
                        for (action, count) in [
                            (ACTION_ADDED, added),
                            (ACTION_SCHEMA_CHANGED, changed.len() - added),
                        ] {
                            if count > 0 {
                                metrics.counter(
                                    EXEC_TOOL_ENTRY_UPDATES_METRIC,
                                    i64::try_from(count).unwrap_or(i64::MAX),
                                    &[("action", action)],
                                );
                            }
                        }
                        metrics.histogram_with_boundaries(
                            EXEC_TOOL_UPDATE_BYTES_METRIC,
                            i64::try_from(update.0.len()).unwrap_or(i64::MAX),
                            TOOL_SPEC_DESCRIPTION_BYTES_METRIC_BUCKETS.as_slice(),
                            &[],
                        );
                    }
                    updates.push(WorldStateUpdate::fragment(update).standalone());
                }
            } else if previous.is_some() {
                // Install the full catalog when enabling granular updates or restoring exec.
                if previous_exec.is_some()
                    && let Some(metrics) = self.metrics.as_deref()
                {
                    metrics.counter(
                        TOOL_INCREMENTAL_UPDATES_METRIC,
                        /*inc*/ 1,
                        &[("action", ACTION_ADDED), ("tool", "functions.exec")],
                    );
                }
                let mut namespace = namespace.clone();
                namespace["tools"] = Value::Array(vec![declaration.clone()]);
                updates.push(WorldStateUpdate::prefix_item(
                    ResponseItem::AdditionalTools {
                        id: None,
                        role: "developer".to_string(),
                        tools: Arc::from(vec![namespace]),
                    },
                ));
            }
            // Removal notices: include exec entries removed while it was unavailable.
            if let Some(previous) = previous_exec {
                let removed = previous
                    .keys()
                    .filter(|key| !exec.contains_key(*key))
                    .cloned()
                    .collect::<Vec<_>>();
                if !removed.is_empty() {
                    if let Some(metrics) = self.metrics.as_deref() {
                        metrics.counter(
                            EXEC_TOOL_ENTRY_UPDATES_METRIC,
                            i64::try_from(removed.len()).unwrap_or(i64::MAX),
                            &[("action", ACTION_REMOVED)],
                        );
                    }
                    updates.push(
                        WorldStateUpdate::fragment(RemovedExecDefinitions {
                            header: self.messages.exec_tools_removed_definitions_header.clone(),
                            names: removed,
                        })
                        .standalone(),
                    );
                }
            }
        } else if let Some(previous) = previous_exec {
            exec = previous.clone();
        }
        exec
    }
}

/// An appended catalog hint or exec definition update, retained or discarded with its baseline.
pub(crate) struct IncrementalToolsUpdate(pub(crate) String);

impl IncrementalToolsUpdate {
    pub(crate) const KIND: &str = "tools.incremental_update";
}

impl ContextualUserFragment for IncrementalToolsUpdate {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind(Self::KIND.to_string())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> ContentItem {
        input_text(self.0.clone())
    }
}

#[derive(Default)]
struct RemovedTools {
    messages: ResolvedIncrementalToolMessages,
    namespaces: Vec<String>,
    tools: Vec<String>,
}

impl ContextualUserFragment for RemovedTools {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("tools.removed_definition".to_string())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    /// Lists removed namespaces and individual tools in separate sections of one notice.
    fn body(&self) -> ContentItem {
        let sections = [
            (
                self.messages.removed_namespaces_header.as_str(),
                &self.namespaces,
            ),
            (self.messages.removed_tools_header.as_str(), &self.tools),
        ];
        let mut body = String::new();
        for (header, names) in sections {
            if names.is_empty() {
                continue;
            }
            if !body.is_empty() {
                body.push_str("\n\n");
            }
            body.push_str(header);
            for name in names {
                body.push_str("\n- ");
                body.push_str(name);
            }
        }
        input_text(body)
    }
}

/// Render a nonempty batch of complete replacement definitions with the exec update hint.
fn render_exec_update(definitions: &[&ExecDefinition], hint: &str) -> IncrementalToolsUpdate {
    let description = definitions
        .iter()
        .map(|entry| entry.description.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut text = hint.to_owned();
    if !text.is_empty() {
        text.push_str("\n\n");
    }
    text.push_str("## Namespace: functions\n");
    text.push_str(&description);
    IncrementalToolsUpdate(text)
}

/// Removal notice containing the stable keys of withdrawn tools or namespace instructions.
struct RemovedExecDefinitions {
    /// Guidance preceding the removed keys; an empty header emits only the keys.
    header: String,
    /// Stable entry keys that are no longer present in the current catalog.
    names: Vec<String>,
}

impl ContextualUserFragment for RemovedExecDefinitions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("tools.removed_definition".to_string())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> ContentItem {
        let mut text = self.header.clone();
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str("- ");
        text.push_str(&self.names.join("\n- "));
        input_text(text)
    }
}

#[cfg(test)]
#[path = "top_level_tools_tests.rs"]
mod tests;
