use super::WorldStateSection;
use super::environment_support::FileSystemContext;
use super::environment_support::NetworkContext;
use super::environment_support::push_xml_escaped_text;
use crate::context::ContextualUserFragment;
use crate::session::turn_context::TurnContext;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnContextNetworkItem;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

/// Live environment values; only identifiers and working directories persist.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct EnvironmentsState {
    environments: BTreeMap<String, EnvironmentState>,
    #[serde(skip)]
    current_date: Option<String>,
    #[serde(skip)]
    timezone: Option<String>,
    #[serde(skip)]
    network: Option<NetworkContext>,
    #[serde(skip)]
    filesystem: Option<FileSystemContext>,
    #[serde(skip)]
    subagents: Option<String>,
    /// Whether the render-only values came from a `TurnContext` or persisted
    /// `TurnContextItem` and can participate in turn-to-turn comparison.
    #[serde(skip)]
    turn_context_values_comparable: bool,
}

impl PartialEq for EnvironmentsState {
    fn eq(&self, other: &Self) -> bool {
        self.environments == other.environments
    }
}

impl Eq for EnvironmentsState {}

impl EnvironmentsState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext) -> Self {
        let mut state = Self {
            environments: turn_context
                .environments
                .turn_environments
                .iter()
                .map(|environment| {
                    (
                        environment.environment_id.clone(),
                        EnvironmentState {
                            cwd: environment.cwd().clone(),
                            status: Some(EnvironmentStatus::Available),
                            shell: environment
                                .shell
                                .as_ref()
                                .map(|shell| shell.name().to_string()),
                        },
                    )
                })
                .collect(),
            current_date: turn_context.current_date.clone(),
            timezone: turn_context.timezone.clone(),
            network: network_from_turn_context(turn_context),
            filesystem: Some(FileSystemContext::from_permission_profile(
                &turn_context.permission_profile,
                &turn_context.config.effective_workspace_roots(),
            )),
            subagents: None,
            turn_context_values_comparable: true,
        };
        for environment in &turn_context.environments.starting {
            state
                .environments
                .entry(environment.selection.environment_id.clone())
                .or_insert_with(|| EnvironmentState {
                    cwd: environment.selection.cwd.clone(),
                    status: Some(EnvironmentStatus::Starting),
                    shell: None,
                });
        }
        state
    }

    pub(crate) fn from_turn_context_item(turn_context_item: &TurnContextItem) -> Self {
        Self {
            environments: [(
                String::new(),
                EnvironmentState {
                    cwd: PathUri::from_abs_path(&turn_context_item.cwd),
                    status: Some(EnvironmentStatus::Available),
                    shell: None,
                },
            )]
            .into_iter()
            .collect(),
            current_date: turn_context_item.current_date.clone(),
            timezone: turn_context_item.timezone.clone(),
            network: network_from_turn_context_item(turn_context_item),
            filesystem: Some(FileSystemContext::from_permission_profile(
                &turn_context_item.permission_profile(),
                &workspace_roots_from_turn_context_item(turn_context_item),
            )),
            subagents: None,
            turn_context_values_comparable: true,
        }
    }

    pub(crate) fn with_subagents(mut self, subagents: String) -> Self {
        if !subagents.is_empty() {
            self.subagents = Some(subagents);
        }
        self
    }

    pub(crate) fn render_diff(&self, previous: &Self) -> Option<ResponseItem> {
        WorldStateSection::render_diff(self, previous)
    }

    fn rendered_full(&self) -> RenderedEnvironments {
        RenderedEnvironments {
            updates: self
                .environments
                .iter()
                .map(|(id, environment)| {
                    (id.clone(), EnvironmentUpdate::Current(environment.clone()))
                })
                .collect(),
            legacy_single: is_legacy_single(&self.environments),
            current_date: self.current_date.clone(),
            timezone: self.timezone.clone(),
            network: self.network.clone(),
            filesystem: self.filesystem.clone(),
            subagents: self.subagents.clone(),
        }
    }
}

impl WorldStateSection for EnvironmentsState {
    const NAME: &'static str = "environments";

    fn render_diff(&self, previous: &Self) -> Option<ResponseItem> {
        let legacy_single =
            is_legacy_single(&self.environments) && previous.environments.len() <= 1;
        let turn_context_values_changed = self.turn_context_values_comparable
            && previous.turn_context_values_comparable
            && (self.current_date != previous.current_date
                || self.timezone != previous.timezone
                || self.network != previous.network
                || self.filesystem != previous.filesystem);
        if legacy_single
            && self.environments.values().next() == previous.environments.values().next()
            && !turn_context_values_changed
        {
            return None;
        }
        let mut updates = self
            .environments
            .iter()
            .filter(|(id, environment)| previous.environments.get(*id) != Some(*environment))
            .map(|(id, environment)| (id.clone(), EnvironmentUpdate::Current(environment.clone())))
            .collect::<BTreeMap<_, _>>();
        if !legacy_single {
            updates.extend(
                previous
                    .environments
                    .keys()
                    .filter(|id| !id.is_empty() && !self.environments.contains_key(*id))
                    .map(|id| (id.clone(), EnvironmentUpdate::Unavailable)),
            );
        }
        (!updates.is_empty() || turn_context_values_changed).then(|| {
            ContextualUserFragment::into(RenderedEnvironments {
                updates,
                legacy_single,
                current_date: self.current_date.clone(),
                timezone: self.timezone.clone(),
                network: self.network.clone(),
                filesystem: self.filesystem.clone(),
                subagents: self.subagents.clone(),
            })
        })
    }
}

impl ContextualUserFragment for EnvironmentsState {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        environment_context_markers()
    }

    fn body(&self) -> String {
        self.rendered_full().body()
    }
}

struct RenderedEnvironments {
    updates: BTreeMap<String, EnvironmentUpdate>,
    legacy_single: bool,
    current_date: Option<String>,
    timezone: Option<String>,
    network: Option<NetworkContext>,
    filesystem: Option<FileSystemContext>,
    subagents: Option<String>,
}

enum EnvironmentUpdate {
    Current(EnvironmentState),
    Unavailable,
}

impl ContextualUserFragment for RenderedEnvironments {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        environment_context_markers()
    }

    fn body(&self) -> String {
        let mut rendered = "\n".to_string();
        if self.legacy_single {
            if let Some(EnvironmentUpdate::Current(environment)) = self.updates.values().next() {
                push_environment_values(&mut rendered, environment, "  ");
            }
        } else if !self.updates.is_empty() {
            rendered.push_str("  <environments>\n");
            for (id, update) in &self.updates {
                match update {
                    EnvironmentUpdate::Current(environment) => {
                        rendered.push_str("    <environment id=\"");
                        push_xml_escaped_text(&mut rendered, id);
                        rendered.push('"');
                        if let Some(status) = environment.status {
                            rendered.push_str(" status=\"");
                            rendered.push_str(status.as_str());
                            rendered.push('"');
                        }
                        rendered.push_str(">\n");
                        push_environment_values(&mut rendered, environment, "      ");
                        rendered.push_str("    </environment>\n");
                    }
                    EnvironmentUpdate::Unavailable => {
                        rendered.push_str("    <environment id=\"");
                        push_xml_escaped_text(&mut rendered, id);
                        rendered.push_str("\" status=\"unavailable\" />\n");
                    }
                }
            }
            rendered.push_str("  </environments>\n");
        }
        push_optional_element(&mut rendered, "current_date", self.current_date.as_deref());
        push_optional_element(&mut rendered, "timezone", self.timezone.as_deref());
        if let Some(network) = &self.network {
            rendered.push_str("  ");
            rendered.push_str(&network.render());
            rendered.push('\n');
        }
        if let Some(filesystem) = &self.filesystem {
            rendered.push_str("  ");
            rendered.push_str(&filesystem.render());
            rendered.push('\n');
        }
        if let Some(subagents) = &self.subagents {
            rendered.push_str("  <subagents>\n");
            for line in subagents.lines() {
                rendered.push_str("    ");
                rendered.push_str(line);
                rendered.push('\n');
            }
            rendered.push_str("  </subagents>\n");
        }
        rendered
    }
}

fn push_environment_values(rendered: &mut String, environment: &EnvironmentState, indent: &str) {
    rendered.push_str(indent);
    rendered.push_str("<cwd>");
    push_xml_escaped_text(rendered, &environment.cwd.inferred_native_path_string());
    rendered.push_str("</cwd>\n");
    if let Some(shell) = &environment.shell {
        rendered.push_str(indent);
        rendered.push_str("<shell>");
        push_xml_escaped_text(rendered, shell);
        rendered.push_str("</shell>\n");
    }
}

fn push_optional_element(rendered: &mut String, name: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    rendered.push_str("  <");
    rendered.push_str(name);
    rendered.push('>');
    push_xml_escaped_text(rendered, value);
    rendered.push_str("</");
    rendered.push_str(name);
    rendered.push_str(">\n");
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EnvironmentState {
    cwd: PathUri,
    #[serde(skip)]
    status: Option<EnvironmentStatus>,
    #[serde(skip)]
    shell: Option<String>,
}

impl PartialEq for EnvironmentState {
    fn eq(&self, other: &Self) -> bool {
        self.cwd == other.cwd
    }
}

impl Eq for EnvironmentState {}

#[derive(Clone, Copy, Debug)]
enum EnvironmentStatus {
    Starting,
    Available,
}

impl EnvironmentStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Available => "available",
        }
    }
}

fn is_legacy_single(environments: &BTreeMap<String, EnvironmentState>) -> bool {
    environments.len() == 1
        && matches!(
            environments
                .values()
                .next()
                .and_then(|environment| environment.status),
            Some(EnvironmentStatus::Available)
        )
}

fn environment_context_markers() -> (&'static str, &'static str) {
    (
        codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG,
        codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG,
    )
}

fn network_from_turn_context(turn_context: &TurnContext) -> Option<NetworkContext> {
    let network = turn_context
        .config
        .config_layer_stack
        .requirements()
        .network
        .as_ref()?;

    Some(NetworkContext::new(
        network
            .domains
            .as_ref()
            .and_then(codex_config::NetworkDomainPermissionsToml::allowed_domains)
            .unwrap_or_default(),
        network
            .domains
            .as_ref()
            .and_then(codex_config::NetworkDomainPermissionsToml::denied_domains)
            .unwrap_or_default(),
    ))
}

fn network_from_turn_context_item(turn_context_item: &TurnContextItem) -> Option<NetworkContext> {
    let TurnContextNetworkItem {
        allowed_domains,
        denied_domains,
    } = turn_context_item.network.as_ref()?;
    Some(NetworkContext::new(
        allowed_domains.clone(),
        denied_domains.clone(),
    ))
}

fn workspace_roots_from_turn_context_item(
    turn_context_item: &TurnContextItem,
) -> Vec<AbsolutePathBuf> {
    if let Some(workspace_roots) = turn_context_item.workspace_roots.as_ref() {
        return workspace_roots.clone();
    }

    vec![turn_context_item.cwd.clone()]
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "environment_render_tests.rs"]
mod render_tests;
