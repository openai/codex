use codex_core_skills::SkillsSnapshot;
use codex_core_skills::render_available_skills_body;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use serde::Deserialize;
use serde::Serialize;

use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;

const REPLACEMENT_NOTICE: &str = "This skills list replaces the previously provided skills list.";
const REMOVAL_NOTICE: &str = "The previously provided skills list no longer applies.";

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct SkillsState {
    skill_root_lines: Vec<String>,
    skill_lines: Vec<String>,
    omitted_count: usize,
}

impl SkillsState {
    pub(crate) fn new(skills: &SkillsSnapshot, enabled: bool) -> Self {
        if !enabled {
            return Self::default();
        }
        let Some(available) = skills.available() else {
            return Self::default();
        };
        Self {
            skill_root_lines: available.skill_root_lines.clone(),
            skill_lines: available.skill_lines.clone(),
            omitted_count: available.report.omitted_count,
        }
    }

    fn has_catalog(&self) -> bool {
        !self.skill_lines.is_empty()
    }
}

impl WorldStateSection for SkillsState {
    const ID: &'static str = "skills";
    type Snapshot = Self;

    fn snapshot(&self) -> Self::Snapshot {
        self.clone()
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        let text = text.trim();
        role == "developer"
            && text.starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
            && text.ends_with(SKILLS_INSTRUCTIONS_CLOSE_TAG)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        let current = self.snapshot();
        if matches!(previous, PreviousSectionState::Known(previous) if previous == &current) {
            return None;
        }
        let previous_had_catalog = match previous {
            PreviousSectionState::Known(previous) => !previous.skill_lines.is_empty(),
            PreviousSectionState::Unknown => true,
            PreviousSectionState::Absent => false,
        };
        if !self.has_catalog() && !previous_had_catalog {
            return None;
        }
        Some(Box::new(SkillsStateFragment {
            skill_root_lines: self.skill_root_lines.clone(),
            skill_lines: self.skill_lines.clone(),
            omitted_count: self.omitted_count,
            include_policy: !previous_had_catalog,
            notice: if !self.has_catalog() {
                Some(REMOVAL_NOTICE)
            } else if previous_had_catalog {
                Some(REPLACEMENT_NOTICE)
            } else {
                None
            },
        }))
    }
}

struct SkillsStateFragment {
    skill_root_lines: Vec<String>,
    skill_lines: Vec<String>,
    omitted_count: usize,
    include_policy: bool,
    notice: Option<&'static str>,
}

impl ContextualUserFragment for SkillsStateFragment {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG)
    }

    fn body(&self) -> String {
        let catalog = (!self.skill_lines.is_empty()).then(|| {
            if self.include_policy {
                render_available_skills_body(&self.skill_root_lines, &self.skill_lines)
            } else {
                let mut lines = vec!["## Skills".to_string()];
                if !self.skill_root_lines.is_empty() {
                    lines.push("### Skill roots".to_string());
                    lines.extend(self.skill_root_lines.iter().cloned());
                }
                lines.push("### Available skills".to_string());
                lines.extend(self.skill_lines.iter().cloned());
                format!("\n{}\n", lines.join("\n"))
            }
        }).map(|mut catalog| {
            if self.omitted_count > 0 {
                catalog.push_str(&format!(
                    "\n{} additional skills are omitted from this model-visible list because of the catalog size limit.\n",
                    self.omitted_count
                ));
            }
            catalog
        });
        match (self.notice, catalog) {
            (Some(notice), Some(catalog)) => format!("\n{notice}\n{catalog}"),
            (Some(notice), None) => format!("\n{notice}\n"),
            (None, Some(catalog)) => catalog,
            (None, None) => String::new(),
        }
    }
}
