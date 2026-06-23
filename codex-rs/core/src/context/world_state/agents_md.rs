use super::WorldStateSection;
use crate::agents_md::AGENTS_MD_SEPARATOR;
use crate::agents_md::EnvironmentInstructions;
use crate::agents_md::LoadedAgentsMd;
use crate::context::ContextualUserFragment;
use crate::context::UserInstructions;
use codex_extension_api::UserInstructions as LoadedUserInstructions;
use codex_utils_path_uri::PathUri;

impl LoadedAgentsMd {
    pub(crate) fn is_empty(&self) -> bool {
        self.base_text().is_none()
            && self
                .environments
                .values()
                .all(EnvironmentInstructions::is_empty)
    }

    /// Returns the concatenated model-visible instruction text.
    pub fn text(&self) -> String {
        if self.has_multiple_project_environments() {
            self.environment_labeled_text()
        } else {
            self.legacy_text()
        }
    }

    pub(crate) fn legacy_text(&self) -> String {
        let mut output = self.base_text().unwrap_or_default();
        let mut has_project_instructions = false;
        for environment in self
            .environments
            .values()
            .filter(|environment| !environment.is_empty())
        {
            if output.is_empty() {
                output.push_str(&environment.text());
            } else if has_project_instructions {
                output.push_str("\n\n");
                output.push_str(&environment.text());
            } else {
                output.push_str(AGENTS_MD_SEPARATOR);
                output.push_str(&environment.text());
            }
            has_project_instructions = true;
        }
        output
    }

    pub(crate) fn environment_labeled_text(&self) -> String {
        let mut sections = self.base_text().into_iter().collect::<Vec<_>>();
        sections.extend(
            self.environments
                .iter()
                .filter(|&(_environment_id, environment)| !environment.is_empty())
                .map(|(environment_id, environment)| {
                    format!(
                        "for `{environment_id}` with root {}\n\n{}",
                        environment.cwd.inferred_native_path_string(),
                        environment.text()
                    )
                }),
        );
        sections.join("\n\n")
    }

    /// Returns the complete model-visible contextual user fragment.
    #[cfg(test)]
    pub(crate) fn render(&self) -> String {
        self.full_fragment().render()
    }

    /// Returns the host-provided user instructions.
    pub(crate) fn user_instructions(&self) -> Option<&LoadedUserInstructions> {
        self.user_instructions.as_ref()
    }

    /// Returns the AGENTS.md files that supplied instruction entries.
    pub fn sources(&self) -> impl Iterator<Item = PathUri> + '_ {
        self.user_instructions
            .iter()
            .map(|instructions| PathUri::from_abs_path(&instructions.source))
            .chain(self.environments.values().flat_map(|environment| {
                environment
                    .entries
                    .iter()
                    .map(|entry| entry.source_path.clone())
            }))
    }

    fn full_fragment(&self) -> UserInstructions {
        let directory = self
            .single_project_environment()
            .map(|environment| environment.cwd.inferred_native_path_string());
        UserInstructions {
            directory,
            text: self.text(),
        }
    }

    fn render_update(&self, previous: &Self) -> Option<UserInstructions> {
        let updates = self
            .environments
            .iter()
            .filter(|(environment_id, environment)| {
                !environment.is_empty() && !previous.environments.contains_key(*environment_id)
            })
            .map(|(environment_id, environment)| {
                format!(
                    "for `{environment_id}` with root {}\n\n{}",
                    environment.cwd.inferred_native_path_string(),
                    environment.text()
                )
            })
            .collect::<Vec<_>>();

        (!updates.is_empty()).then(|| UserInstructions {
            directory: None,
            text: updates.join("\n\n"),
        })
    }

    fn base_text(&self) -> Option<String> {
        let sections = self
            .user_instructions
            .iter()
            .map(|instructions| instructions.text.as_str())
            .chain(self.internal_instructions.iter().map(String::as_str))
            .filter(|instructions| !instructions.trim().is_empty())
            .collect::<Vec<_>>();
        (!sections.is_empty()).then(|| sections.join("\n\n"))
    }

    fn has_multiple_project_environments(&self) -> bool {
        self.environments
            .values()
            .filter(|environment| !environment.is_empty())
            .nth(1)
            .is_some()
    }

    fn single_project_environment(&self) -> Option<&EnvironmentInstructions> {
        let mut environments = self
            .environments
            .values()
            .filter(|environment| !environment.is_empty());
        let environment = environments.next()?;
        environments.next().is_none().then_some(environment)
    }
}

impl EnvironmentInstructions {
    fn is_empty(&self) -> bool {
        self.entries
            .iter()
            .all(|entry| entry.contents.trim().is_empty())
    }

    fn text(&self) -> String {
        self.entries
            .iter()
            .map(|entry| entry.contents.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl WorldStateSection for LoadedAgentsMd {
    fn render_diff(&self, previous: Option<&Self>) -> Option<Box<dyn ContextualUserFragment>> {
        let fragment = match previous {
            Some(previous) => self.render_update(previous),
            None if self.is_empty() => None,
            None => Some(self.full_fragment()),
        }?;
        Some(Box::new(fragment))
    }
}
