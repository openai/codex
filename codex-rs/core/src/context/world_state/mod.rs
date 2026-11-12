mod agents_md;
mod apps_instructions;
mod collaboration_mode;
mod command_sessions;
mod compact_permissions;
mod context_window_guidance;
mod environment;
mod environments_instructions;
mod managed_developer_instructions;
mod model;
mod model_catalog;
mod multi_agent_mode;
mod multi_agent_usage_hint;
mod permissions;
mod persistent_mode;
mod plugins_instructions;
mod realtime;
#[cfg(test)]
pub(crate) mod test_support;
mod tools;
mod top_level_tools;

use crate::AgentSessionReader;
use crate::context::ContextualUserFragment;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseItem;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Map;
use serde_json::Value;
use sha1::Digest;
use sha1::Sha1;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

pub(crate) use agents_md::AgentsMdState;
pub(crate) use apps_instructions::AppsInstructionsState;
pub(crate) use collaboration_mode::CollaborationModeState;
pub(crate) use command_sessions::CommandSession;
pub(crate) use command_sessions::CommandSessionStatus;
pub(crate) use command_sessions::CommandSessionsState;
pub(crate) use command_sessions::DisabledCommandSessionsState;
pub(crate) use compact_permissions::CompactPermissionsState;
pub(crate) use context_window_guidance::ContextWindowGuidanceState;
pub(crate) use environment::EnvironmentsState;
use environment::RenderedEnvironments;
pub(crate) use environment::format_agent_paths;
pub(crate) use environments_instructions::EnvironmentsInstructionsState;
pub(crate) use managed_developer_instructions::ManagedDeveloperInstructions;
pub(crate) use managed_developer_instructions::ManagedDeveloperInstructionsState;
pub(crate) use managed_developer_instructions::validate_managed_developer_instructions;
pub(crate) use model::ModelInstructionsState;
pub(crate) use model_catalog::ModelCatalogState;
pub(crate) use multi_agent_mode::MultiAgentModeState;
pub(crate) use multi_agent_usage_hint::MultiAgentUsageHintState;
pub(crate) use permissions::PermissionsState;
pub(crate) use persistent_mode::PersistentModeState;
pub(crate) use plugins_instructions::PluginsInstructionsState;
pub(crate) use realtime::RealtimeState;
pub(crate) use tools::ToolsState;
pub(crate) use top_level_tools::IncrementalToolsUpdate;
pub(crate) use top_level_tools::TopLevelToolsState;

/// One contribution to model context and its placement policy.
pub(crate) struct WorldStateUpdate {
    pub(crate) placement: Placement,
    pub(crate) content: WorldStateUpdateContent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    /// A separate item placed at the front when assembling initial context.
    Prefix,
    Standalone,
    Mergeable,
}

pub(crate) type WorldStateFragment = Box<dyn ContextualUserFragment + Send>;

pub(crate) enum WorldStateUpdateContent {
    Fragment(WorldStateFragment),
    Environment(Box<RenderedEnvironments>),
    Item(Box<ResponseItem>),
}

impl WorldStateUpdateContent {
    pub(crate) fn into_fragment(self) -> Result<WorldStateFragment, Box<ResponseItem>> {
        match self {
            Self::Fragment(fragment) => Ok(fragment),
            Self::Environment(environment) => Ok(environment),
            Self::Item(item) => Err(item),
        }
    }
}

impl WorldStateUpdate {
    pub(crate) fn fragment(fragment: impl ContextualUserFragment + Send + 'static) -> Self {
        Self::boxed_fragment(Box::new(fragment))
    }

    fn boxed_fragment(fragment: WorldStateFragment) -> Self {
        Self {
            placement: Placement::Mergeable,
            content: WorldStateUpdateContent::Fragment(fragment),
        }
    }

    pub(crate) fn optional_boxed_fragment(fragment: Option<WorldStateFragment>) -> Vec<Self> {
        fragment.into_iter().map(Self::boxed_fragment).collect()
    }

    pub(crate) fn optional_standalone_boxed_fragment(
        fragment: Option<WorldStateFragment>,
    ) -> Vec<Self> {
        fragment
            .into_iter()
            .map(Self::boxed_fragment)
            .map(Self::standalone)
            .collect()
    }

    pub(crate) fn standalone(mut self) -> Self {
        self.placement = Placement::Standalone;
        self
    }

    pub(crate) fn prefix_item(item: ResponseItem) -> Self {
        Self {
            placement: Placement::Prefix,
            content: WorldStateUpdateContent::Item(Box::new(item)),
        }
    }
}

pub(crate) type SectionTransition<S = Value> = (Option<S>, Vec<WorldStateUpdate>);

/// Separates the rendered window prefix from the remaining initial-context updates.
pub(crate) fn split_prefix_updates(
    updates: Vec<WorldStateUpdate>,
) -> (Vec<ResponseItem>, Vec<WorldStateUpdate>) {
    let mut prefix = Vec::new();
    let mut context = Vec::new();
    for update in updates {
        match update.placement {
            Placement::Prefix => prefix.push(match update.content.into_fragment() {
                Ok(fragment) => fragment.into_boxed_response_item(),
                Err(item) => *item,
            }),
            Placement::Standalone | Placement::Mergeable => context.push(update),
        }
    }
    (prefix, context)
}

trait ErasedWorldStateSection: Send + Sync {
    fn include_in_full_context(&self) -> bool {
        true
    }

    fn matches_legacy_fragment(&self, role: &str, text: &str) -> bool;

    fn has_retained_fragment_matcher(&self) -> bool;

    fn matches_retained_fragment(&self, role: &str, text: &str) -> bool;

    fn needs_latest_fragment(&self) -> bool {
        false
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Value>,
        retained: Option<&str>,
    ) -> SectionTransition;
}

fn section_snapshot<S: WorldStateSection>(section: &S, snapshot: S::Snapshot) -> Option<Value> {
    if !WorldStateSection::should_persist(section) {
        return None;
    }
    let mut snapshot = match serde_json::to_value(snapshot) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            tracing::error!(
                section_id = S::ID,
                %err,
                "failed to serialize world-state section snapshot"
            );
            return None;
        }
    };
    remove_null_object_fields(&mut snapshot);
    if snapshot.is_null() {
        tracing::error!(
            section_id = S::ID,
            "world-state section snapshot cannot be null"
        );
        return None;
    }
    Some(snapshot)
}

impl<S: WorldStateSection> ErasedWorldStateSection for S {
    fn matches_legacy_fragment(&self, role: &str, text: &str) -> bool {
        WorldStateSection::matches_current_legacy_fragment(self, role, text)
    }

    fn has_retained_fragment_matcher(&self) -> bool {
        WorldStateSection::has_current_retained_fragment_matcher(self)
    }

    fn matches_retained_fragment(&self, role: &str, text: &str) -> bool {
        S::matches_retained_fragment(role, text)
    }

    fn needs_latest_fragment(&self) -> bool {
        S::NEEDS_LATEST_FRAGMENT
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Value>,
        retained: Option<&str>,
    ) -> SectionTransition {
        let typed_snapshot;
        let previous = match previous {
            PreviousSectionState::Known(previous) => {
                // Deserialize the borrowed snapshot without copying its JSON tree.
                match S::Snapshot::deserialize(previous) {
                    Ok(previous) => {
                        typed_snapshot = previous;
                        PreviousSectionState::Known(&typed_snapshot)
                    }
                    Err(err) => {
                        tracing::warn!(
                            section_id = S::ID,
                            %err,
                            "failed to restore world-state section snapshot"
                        );
                        PreviousSectionState::Unknown
                    }
                }
            }
            PreviousSectionState::Absent => PreviousSectionState::Absent,
            PreviousSectionState::Unknown => PreviousSectionState::Unknown,
        };
        let (snapshot, updates) = self.render_retained_diff(previous, retained);
        (
            snapshot.map(|snapshot| section_snapshot(self, snapshot).unwrap_or(Value::Null)),
            updates,
        )
    }
}

struct ExtensionWorldStateSection(WorldStateSectionContribution);

impl ErasedWorldStateSection for ExtensionWorldStateSection {
    fn include_in_full_context(&self) -> bool {
        self.0.include_in_full_context()
    }

    fn matches_legacy_fragment(&self, role: &str, text: &str) -> bool {
        self.0.matches_legacy_fragment(role, text)
    }

    fn has_retained_fragment_matcher(&self) -> bool {
        self.0.has_retained_fragment_matcher()
    }

    fn matches_retained_fragment(&self, role: &str, text: &str) -> bool {
        self.0.matches_retained_fragment(role, text)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Value>,
        _retained: Option<&str>,
    ) -> SectionTransition {
        let previous = match previous {
            PreviousSectionState::Absent => PreviousWorldStateSection::Absent,
            PreviousSectionState::Unknown => PreviousWorldStateSection::Unknown,
            PreviousSectionState::Known(previous) => PreviousWorldStateSection::Known(previous),
        };
        let (snapshot, fragment) = self.0.render_diff(previous);
        let snapshot = snapshot.map(|mut snapshot| {
            remove_null_object_fields(&mut snapshot);
            snapshot
        });
        let updates = fragment
            .into_iter()
            .filter(|fragment| !self.0.is_window_prefix() || fragment.role() == "developer")
            .map(|fragment| {
                let update = WorldStateUpdate::fragment(WorldStateContextFragment {
                    fragment,
                    content_kind: ContentItemKind(format!("{}.instructions", self.0.id())),
                });
                if self.0.is_window_prefix() {
                    WorldStateUpdate {
                        placement: Placement::Prefix,
                        ..update
                    }
                } else {
                    update
                }
            })
            .collect();
        (snapshot, updates)
    }
}

struct WorldStateContextFragment {
    fragment: RenderedWorldStateFragment,
    content_kind: ContentItemKind,
}

impl ContextualUserFragment for WorldStateContextFragment {
    fn content_metadata(&self) -> codex_protocol::models::ContentItemMetadata {
        self.fragment.metadata().clone()
    }

    fn content_kind(&self) -> ContentItemKind {
        self.content_kind.clone()
    }

    fn role(&self) -> &'static str {
        self.fragment.role()
    }

    fn markers(&self) -> (&'static str, &'static str) {
        self.fragment.markers()
    }

    fn body(&self) -> ContentItem {
        self.fragment.body().clone()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }
}

/// What is known about a section's previously model-visible state.
pub(crate) enum PreviousSectionState<'a, T> {
    /// No persisted snapshot or matching fragment exists in retained history.
    Absent,
    /// Retained history contains the section, but its typed snapshot is unavailable.
    Unknown,
    /// The exact persisted snapshot is available.
    Known(&'a T),
}

/// A typed portion of the state visible to the model.
///
/// Implementations own how their current state is rendered relative to an
/// earlier snapshot of the same section. `ID` is persisted in rollouts and
/// must remain stable. `Snapshot` should contain only the comparison data
/// needed to decide what the model must be told next, and must not serialize
/// to null because merge-patch nulls represent deletion. Sections migrated
/// from older context can recognize their previous fragments through
/// `matches_legacy_fragment`.
pub(crate) trait WorldStateSection: Send + Sync + 'static {
    const ID: &'static str;
    type Snapshot: DeserializeOwned + Serialize;

    /// Whether history recovery needs the most recent match instead of any match.
    const NEEDS_LATEST_FRAGMENT: bool = false;

    fn render_retained_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
        _retained: Option<&str>,
    ) -> SectionTransition<Self::Snapshot> {
        self.render_diff(previous)
    }

    /// Whether the section contributes comparison state to persisted rollouts.
    fn should_persist(&self) -> bool {
        true
    }

    fn matches_legacy_fragment(_role: &str, _text: &str) -> bool {
        false
    }

    /// Recognizes legacy fragments whose identity depends on this section's current value.
    fn matches_current_legacy_fragment(&self, role: &str, text: &str) -> bool {
        Self::matches_legacy_fragment(role, text)
    }

    /// Whether retained history must still contain this section's rendered fragment.
    fn has_retained_fragment_matcher() -> bool {
        false
    }

    /// Whether this instance requires its rendered fragment to survive in history.
    fn has_current_retained_fragment_matcher(&self) -> bool {
        Self::has_retained_fragment_matcher()
    }

    /// Recognizes this section's rendered fragment in retained model history.
    fn matches_retained_fragment(_role: &str, _text: &str) -> bool {
        false
    }

    /// Returns an optional snapshot update and ordered model-context updates.
    /// A missing snapshot leaves the stored comparison state unchanged.
    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot>;
}

/// Stable fingerprint of model-visible world-state content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct WorldStateHash(String);

impl WorldStateHash {
    pub(crate) fn from_json(value: &Value) -> Self {
        let mut value = value.clone();
        value.sort_all_objects();
        Self(format!("{:x}", Sha1::digest(value.to_string().as_bytes())))
    }

    pub(crate) fn from_fragment(fragment: &(impl ContextualUserFragment + ?Sized)) -> Self {
        let mut hasher = Sha1::new();
        match fragment.render() {
            ContentItem::InputText { text } => {
                hasher.update(b"codex-world-state-fragment-v1\0");
                hash_component(&mut hasher, fragment.role());
                hash_component(&mut hasher, &text);
            }
            content => {
                let content = serde_json::to_vec(&content)
                    .unwrap_or_else(|_| unreachable!("ContentItem always serializes"));
                hasher.update(b"codex-world-state-fragment-v2\0");
                hash_component(&mut hasher, fragment.role());
                hasher.update((content.len() as u64).to_be_bytes());
                hasher.update(content);
            }
        }
        Self(format!("{:x}", hasher.finalize()))
    }
}

fn hash_component(hasher: &mut Sha1, value: &str) {
    let value = value.replace("\r\n", "\n");
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

/// Live model-visible state, keyed by the same stable section IDs used in rollouts.
#[derive(Default)]
pub(crate) struct WorldState {
    pub(crate) host_roster_reader: Option<Arc<dyn AgentSessionReader>>,
    sections: IndexMap<&'static str, Box<dyn ErasedWorldStateSection>>,
}

/// Compact comparison state for each model-visible world-state section.
#[derive(Clone, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct WorldStateSnapshot {
    sections: BTreeMap<String, Value>,
    // Rendering may need a refresh while persistence still needs the exact prior state.
    #[serde(skip)]
    invalidated_sections: BTreeSet<String>,
}

impl From<&Map<String, Value>> for WorldStateSnapshot {
    fn from(state: &Map<String, Value>) -> Self {
        Self {
            sections: state
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            invalidated_sections: BTreeSet::new(),
        }
    }
}

impl WorldStateSnapshot {
    pub(crate) fn remove_section(&mut self, id: &str) {
        self.sections.remove(id);
        self.invalidated_sections.remove(id);
    }

    pub(crate) fn invalidate_section(&mut self, id: &str) {
        self.invalidated_sections.insert(id.to_string());
    }

    pub(crate) fn into_object(self) -> Map<String, Value> {
        self.sections.into_iter().collect()
    }

    fn section(&self, id: &str) -> Option<&Value> {
        if self.invalidated_sections.contains(id) {
            return None;
        }
        let snapshot = self.sections.get(id)?;
        if id == PluginsInstructionsState::ID
            && snapshot == &Value::Bool(false)
            && let Some(retired) = self
                .sections
                .get(plugins_instructions::RETIRED_CHECKPOINT_ID)
            && plugins_instructions::is_retired_checkpoint(retired)
        {
            return Some(retired);
        }
        Some(snapshot)
    }

    fn insert_section(&mut self, id: &str, mut snapshot: Value) {
        if id == PluginsInstructionsState::ID
            && plugins_instructions::is_retired_checkpoint(&snapshot)
        {
            // Keep the original section readable by clients whose snapshot type is bool.
            self.sections.insert(
                plugins_instructions::RETIRED_CHECKPOINT_ID.to_string(),
                snapshot,
            );
            snapshot = Value::Bool(false);
        }
        self.sections.insert(id.to_string(), snapshot);
    }

    /// Returns the RFC 7386 merge patch that advances `previous` to `self`.
    pub(crate) fn merge_patch_from(&self, previous: &Self) -> Option<Map<String, Value>> {
        let mut patch = Map::new();
        // Emit removals first to preserve insertion-ordered JSON patch output.
        for key in previous.sections.keys() {
            if !self.sections.contains_key(key) {
                patch.insert(key.clone(), Value::Null);
            }
        }
        for (key, current) in &self.sections {
            if let Some(previous) = previous.sections.get(key) {
                if let Some(value) = create_merge_patch(previous, current) {
                    patch.insert(key.clone(), value);
                }
            } else {
                patch.insert(key.clone(), current.clone());
            }
        }
        (!patch.is_empty()).then_some(patch)
    }

    pub(crate) fn apply_merge_patch(&mut self, patch: &Map<String, Value>) {
        // Borrow existing keys; only newly inserted sections need owned keys.
        for (key, value) in patch {
            self.invalidated_sections.remove(key);
            if value.is_null() {
                self.sections.remove(key);
            } else if let Some(current) = self.sections.get_mut(key) {
                apply_merge_patch_value(current, value);
            } else {
                let mut current = Value::Null;
                apply_merge_patch_value(&mut current, value);
                self.sections.insert(key.clone(), current);
            }
        }
    }
}

impl fmt::Debug for WorldState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldState")
            .field("section_count", &self.sections.len())
            .finish()
    }
}

enum RenderScope {
    FullContext,
    HistoryDiff,
}

impl WorldState {
    /// Read the roster only when rendering an environment fragment. If the read fails,
    /// keep rendering the rest of the fragment.
    pub(crate) async fn resolve_host_roster(&self, updates: &mut [WorldStateUpdate]) {
        if let Some(reader) = &self.host_roster_reader
            && let Some(environment) =
                updates
                    .iter_mut()
                    .find_map(|update| match &mut update.content {
                        WorldStateUpdateContent::Environment(environment) => Some(environment),
                        WorldStateUpdateContent::Fragment(_) | WorldStateUpdateContent::Item(_) => {
                            None
                        }
                    })
        {
            match reader.children().await {
                Ok(children) => {
                    let roster = format_agent_paths(children.into_iter().map(|agent| agent.path));
                    environment.subagents = (!roster.is_empty()).then_some(roster);
                }
                Err(_) => tracing::warn!("failed to read host agent roster for prompt context"),
            }
        }
    }

    pub(crate) fn full_context_excluded_section_ids(
        &self,
    ) -> impl Iterator<Item = &'static str> + '_ {
        self.sections
            .iter()
            .filter_map(|(id, section)| (!section.include_in_full_context()).then_some(*id))
    }

    pub(crate) fn add_section<S: WorldStateSection>(&mut self, section: S) {
        let id = S::ID;
        assert!(
            id != plugins_instructions::RETIRED_CHECKPOINT_ID && !self.sections.contains_key(id),
            "duplicate world-state section ID: {id}"
        );
        self.sections.insert(id, Box::new(section));
    }

    pub(crate) fn add_extension_section(&mut self, section: WorldStateSectionContribution) {
        let id = section.id();
        assert!(
            id != plugins_instructions::RETIRED_CHECKPOINT_ID && !self.sections.contains_key(id),
            "duplicate world-state section ID: {id}"
        );
        let section = Box::new(ExtensionWorldStateSection(section));
        if id == "host_skills"
            && let Some(index) = self.sections.get_index_of(PermissionsState::ID)
        {
            self.sections.shift_insert(index, id, section);
        } else {
            self.sections.insert(id, section);
        }
    }

    /// Renders restorable sections as new, preserving update order and placement.
    pub(crate) async fn render_full_with_host_roster(
        &self,
    ) -> (WorldStateSnapshot, Vec<WorldStateUpdate>) {
        let (snapshot, mut updates) = self.render_full();
        self.resolve_host_roster(&mut updates).await;
        (snapshot, updates)
    }

    pub(crate) fn render_full(&self) -> (WorldStateSnapshot, Vec<WorldStateUpdate>) {
        self.render_with(RenderScope::FullContext, /*stored*/ None, |_, _| {
            (PreviousSectionState::Absent, None)
        })
    }

    /// Falls back to retained model history when no exact persisted snapshot is available.
    pub(crate) fn render_history_diff<'a>(
        &self,
        previous: Option<&WorldStateSnapshot>,
        items: impl IntoIterator<Item = &'a ResponseItem, IntoIter: DoubleEndedIterator> + Clone,
    ) -> (WorldStateSnapshot, Vec<WorldStateUpdate>) {
        self.render_with(RenderScope::HistoryDiff, previous, |id, section| {
            let stored = previous.and_then(|previous| previous.section(id));
            if let Some(stored) = stored
                && !section.has_retained_fragment_matcher()
            {
                return (PreviousSectionState::Known(stored), None);
            }
            let matches = |role: &str, text: &str| {
                if stored.is_some() {
                    section.matches_retained_fragment(role, text)
                } else {
                    section.matches_legacy_fragment(role, text)
                }
            };
            let mut fragments = items
                .clone()
                .into_iter()
                .filter_map(|item| match item {
                    ResponseItem::Message { role, content, .. } => Some((role, content)),
                    _ => None,
                })
                .flat_map(|(role, content)| {
                    content.iter().filter_map(move |item| match item {
                        ContentItem::InputText { text } if matches(role, text) => {
                            Some(text.as_str())
                        }
                        _ => None,
                    })
                });
            let retained = if section.needs_latest_fragment() {
                fragments.next_back()
            } else {
                fragments.next()
            };
            let state = match (stored, retained) {
                (_, None) => PreviousSectionState::Absent,
                (None, Some(_)) => PreviousSectionState::Unknown,
                (Some(stored), Some(_)) => PreviousSectionState::Known(stored),
            };
            (state, retained)
        })
    }

    fn render_with<'a>(
        &self,
        scope: RenderScope,
        stored: Option<&WorldStateSnapshot>,
        mut previous: impl FnMut(
            &str,
            &dyn ErasedWorldStateSection,
        ) -> (PreviousSectionState<'a, Value>, Option<&'a str>),
    ) -> (WorldStateSnapshot, Vec<WorldStateUpdate>) {
        let mut snapshot = WorldStateSnapshot::default();
        let mut updates = Vec::new();
        for (id, section) in &self.sections {
            if matches!(scope, RenderScope::FullContext) && !section.include_in_full_context() {
                continue;
            }
            let (prior, retained) = previous(id, section.as_ref());
            let (section_snapshot, section_updates) = section.render_diff(prior, retained);
            // A skipped snapshot retains stored state even if history required a fresh render.
            let section_snapshot = section_snapshot
                .or_else(|| stored.and_then(|snapshot| snapshot.section(id)).cloned());
            if let Some(section_snapshot) = section_snapshot
                && !section_snapshot.is_null()
            {
                snapshot.insert_section(id, section_snapshot);
            }
            updates.extend(section_updates);
        }
        (snapshot, updates)
    }
}

fn remove_null_object_fields(value: &mut Value) {
    // RFC 7386 reserves object-valued nulls for deletion, but arrays are replaced whole.
    match value {
        Value::Object(values) => {
            values.retain(|_, value| !value.is_null());
            values.values_mut().for_each(remove_null_object_fields);
        }
        Value::Array(_) => {}
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn create_merge_patch(previous: &Value, current: &Value) -> Option<Value> {
    if previous == current {
        return None;
    }

    let Value::Object(current) = current else {
        return Some(current.clone());
    };
    let previous = previous.as_object();
    let mut patch = Map::new();

    if let Some(previous) = previous {
        for key in previous.keys() {
            if !current.contains_key(key) {
                patch.insert(key.clone(), Value::Null);
            }
        }
    }

    for (key, current_value) in current {
        let Some(previous_value) = previous.and_then(|previous| previous.get(key)) else {
            patch.insert(key.clone(), current_value.clone());
            continue;
        };
        if let Some(value_patch) = create_merge_patch(previous_value, current_value) {
            patch.insert(key.clone(), value_patch);
        }
    }

    Some(Value::Object(patch))
}

fn apply_merge_patch_value(target: &mut Value, patch: &Value) {
    // Nested patches can replace objects with scalars or arrays.
    let Value::Object(patch) = patch else {
        target.clone_from(patch);
        return;
    };
    // RFC 7386 replaces non-object values with an object before merging.
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    if let Value::Object(target) = target {
        for (key, value) in patch {
            if value.is_null() {
                target.remove(key);
            } else if let Some(current) = target.get_mut(key) {
                apply_merge_patch_value(current, value);
            } else {
                let mut current = Value::Null;
                apply_merge_patch_value(&mut current, value);
                target.insert(key.clone(), current);
            }
        }
    }
}

#[cfg(test)]
#[path = "world_state_tests.rs"]
mod tests;
