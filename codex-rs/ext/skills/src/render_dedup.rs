//! Prefers cloud skills before budgeting while retaining aliases from the full visible inventory.

use std::collections::HashSet;

use super::AvailableSkillsRender;
use super::CatalogLines;
use super::SkillCatalogRenderPolicy;
use super::SkillMetadataBudget;
use super::aliased_render_is_better;
use super::build_alias_plan;
use super::build_aliased_catalog;
use super::render_catalog;
use crate::aliases::AliasPlan;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog_prompt::SkillPromptKind;

pub(crate) struct PreparedSkillCatalog<'a> {
    pub(super) entries: Vec<&'a SkillCatalogEntry>,
    alias_plan: Option<AliasPlan>,
    policy: SkillCatalogRenderPolicy,
}

impl<'a> PreparedSkillCatalog<'a> {
    pub(crate) fn new(catalog: &'a SkillCatalog, policy: SkillCatalogRenderPolicy) -> Self {
        let mut entries = catalog
            .entries
            .iter()
            .filter(|entry| entry.is_model_visible())
            .collect::<Vec<_>>();
        policy.order_entries(&mut entries);
        let alias_plan = build_alias_plan(&entries);
        Self {
            entries,
            alias_plan,
            policy,
        }
    }

    pub(crate) fn prefer_cloud_skills(&mut self, cloud: &SkillCatalog) {
        let cloud_names: HashSet<_> = cloud
            .entries
            .iter()
            .filter(|entry| entry.is_model_visible())
            .map(|entry| entry.name.as_str())
            .filter(|name| {
                name.split_once(':')
                    .is_some_and(|(plugin, skill)| !plugin.is_empty() && !skill.is_empty())
            })
            .collect();

        self.entries
            .retain(|entry| !cloud_names.contains(entry.name.as_str()));
    }

    pub(super) fn unaliased(&self) -> CatalogLines<'a> {
        CatalogLines::unaliased(&self.entries, self.policy)
    }

    pub(super) fn aliased(&self) -> CatalogLines<'a> {
        CatalogLines::aliased(&self.entries, self.policy, self.alias_plan.as_ref())
    }

    pub(crate) fn render(
        &self,
        budget: SkillMetadataBudget,
        include_skills_usage_instructions: bool,
    ) -> Option<AvailableSkillsRender> {
        if self.entries.is_empty() {
            return None;
        }
        let absolute = render_catalog(
            self.unaliased().skills,
            budget,
            Vec::new(),
            SkillPromptKind::Unaliased,
            self.policy,
        );
        let selected = if let Some(aliased) = build_aliased_catalog(
            self.aliased(),
            self.policy,
            budget,
            include_skills_usage_instructions,
        ) && aliased_render_is_better(
            &aliased,
            &absolute,
            budget,
            include_skills_usage_instructions,
        ) {
            aliased
        } else {
            absolute
        };

        Some(AvailableSkillsRender {
            prompt_kind: selected.prompt_kind,
            skill_root_lines: selected.skill_root_lines,
            skill_lines: selected.skill_lines,
            preserve_empty_fragment: self.policy == SkillCatalogRenderPolicy::CoreCompatible,
            report: selected.report,
        })
    }
}
