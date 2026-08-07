use std::collections::HashSet;

use codex_extension_api::ContextualUserFragment;
use codex_skills::SkillMetadata;
use codex_skills::normalize_skill_path;

use crate::HostSkillsSnapshot;
use crate::fragments::SkillInstructions;
use crate::render::truncate_main_prompt_contents;

/// Host skill prompts already supplied or superseded by an extension.
///
/// Core preserves its host skill invocation lifecycle while avoiding duplicate
/// prompts and retaining executor/orchestrator skill precedence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InjectedHostSkillPrompts {
    paths: HashSet<String>,
}

impl InjectedHostSkillPrompts {
    pub fn insert_path(&mut self, path: impl Into<String>) {
        let path = path.into();
        self.paths.insert(normalize_host_skill_path(&path));
        self.paths.insert(path);
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn contains_path(&self, path: &str) -> bool {
        self.paths.contains(path) || self.paths.contains(&normalize_host_skill_path(path))
    }
}

/// Prompt fragments and read outcomes for a set of selected host skills.
pub struct HostSkillPrompts {
    pub fragments: Vec<Box<dyn ContextualUserFragment + Send>>,
    pub injected: Vec<SkillMetadata>,
    pub warnings: Vec<String>,
}

fn normalize_host_skill_path(path: &str) -> String {
    normalize_skill_path(path).replace('\\', "/")
}

impl HostSkillsSnapshot {
    /// Reads selected host skills and builds their model-visible prompt fragments.
    ///
    /// Core calls this directly, including for hosts without an installed skills extension.
    #[tracing::instrument(
        level = "trace",
        skip_all,
        fields(selected_skill_count = selected_skills.len())
    )]
    pub async fn load_skill_prompts(&self, selected_skills: &[SkillMetadata]) -> HostSkillPrompts {
        let mut prompts = HostSkillPrompts {
            fragments: Vec::with_capacity(selected_skills.len()),
            injected: Vec::with_capacity(selected_skills.len()),
            warnings: Vec::new(),
        };

        for skill in selected_skills {
            match self.read_skill_text(skill).await {
                Ok(contents) => {
                    let (contents, truncated) = if self.outcome().is_agent_plugin_skill(skill) {
                        truncate_main_prompt_contents(&contents)
                    } else {
                        (contents, false)
                    };
                    if truncated {
                        prompts.warnings.push(format!(
                            "Skill `{}` exceeded the main prompt context limit and was truncated.",
                            skill.name
                        ));
                    }
                    prompts.fragments.push(Box::new(SkillInstructions {
                        name: skill.name.clone(),
                        path: skill.path_to_skills_md.to_string_lossy().into_owned(),
                        contents,
                        resource_access: None,
                    }));
                    prompts.injected.push(skill.clone());
                }
                Err(err) => {
                    prompts.warnings.push(format!(
                        "Failed to load skill {} at {}: {err:#}",
                        skill.name,
                        skill.path_to_skills_md.display()
                    ));
                }
            }
        }

        prompts
    }
}
