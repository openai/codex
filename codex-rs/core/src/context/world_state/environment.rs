use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::environment_context::EnvironmentContext;
use crate::context::environment_context::EnvironmentContextEnvironment;
use crate::context::environment_context::EnvironmentContextEnvironments;
use crate::context::environment_context::FileSystemContext;
use crate::context::environment_context::NetworkContext;
use crate::context::environment_context::push_xml_escaped_text;
use crate::session::turn_context::TurnContext;
use crate::shell::Shell;
use codex_protocol::models::ResponseItem;
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
}

impl PartialEq for EnvironmentsState {
    fn eq(&self, other: &Self) -> bool {
        self.environments == other.environments
    }
}

impl Eq for EnvironmentsState {}

impl EnvironmentsState {
    pub(crate) fn from_turn_context(
        turn_context: &TurnContext,
        default_shell: &Shell,
        subagents: String,
    ) -> Self {
        let context = EnvironmentContext::from_turn_context(turn_context, default_shell)
            .with_subagents(subagents);
        let mut state = Self::from_environment_context(&context);
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

    pub(crate) fn from_environment_context(context: &EnvironmentContext) -> Self {
        let mut environments = BTreeMap::new();
        match &context.environments {
            EnvironmentContextEnvironments::None => {}
            EnvironmentContextEnvironments::Single(environment) => {
                insert_environment(&mut environments, environment);
            }
            EnvironmentContextEnvironments::Multiple(current) => {
                for environment in current {
                    insert_environment(&mut environments, environment);
                }
            }
        }
        Self {
            environments,
            current_date: context.current_date.clone(),
            timezone: context.timezone.clone(),
            network: context.network.clone(),
            filesystem: context.filesystem.clone(),
            subagents: context.subagents.clone(),
        }
    }
}

impl WorldStateSection for EnvironmentsState {
    const NAME: &'static str = "environments";

    fn render_diff(&self, previous: &Self) -> Option<ResponseItem> {
        let mut updates = self
            .environments
            .iter()
            .filter(|(id, environment)| previous.environments.get(*id) != Some(*environment))
            .map(|(id, environment)| (id.clone(), EnvironmentUpdate::Current(environment.clone())))
            .collect::<BTreeMap<_, _>>();
        updates.extend(
            previous
                .environments
                .keys()
                .filter(|id| !self.environments.contains_key(*id))
                .map(|id| (id.clone(), EnvironmentUpdate::Unavailable)),
        );
        (!updates.is_empty()).then(|| {
            ContextualUserFragment::into(RenderedEnvironments {
                updates,
                current_date: self.current_date.clone(),
                timezone: self.timezone.clone(),
                network: self.network.clone(),
                filesystem: self.filesystem.clone(),
                subagents: self.subagents.clone(),
            })
        })
    }
}

struct RenderedEnvironments {
    updates: BTreeMap<String, EnvironmentUpdate>,
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
        (
            codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG,
            codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG,
        )
    }

    fn body(&self) -> String {
        let mut rendered = "\n".to_string();
        if !self.updates.is_empty() {
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
                        rendered.push_str(">\n      <cwd>");
                        push_xml_escaped_text(
                            &mut rendered,
                            &environment.cwd.inferred_native_path_string(),
                        );
                        rendered.push_str("</cwd>");
                        if let Some(shell) = &environment.shell {
                            rendered.push_str("\n      <shell>");
                            push_xml_escaped_text(&mut rendered, shell);
                            rendered.push_str("</shell>");
                        }
                        rendered.push_str("\n    </environment>\n");
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

fn insert_environment(
    environments: &mut BTreeMap<String, EnvironmentState>,
    environment: &EnvironmentContextEnvironment,
) {
    environments.insert(
        environment.id.clone(),
        EnvironmentState {
            cwd: environment.cwd.clone(),
            status: Some(EnvironmentStatus::Available),
            shell: Some(environment.shell.clone()),
        },
    );
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

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
