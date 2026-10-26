//! Prefers cloud skills before budgeting while retaining aliases from the full visible inventory.
//! Collects comparison metadata after the final rendering has been selected.

use std::borrow::Cow;
use std::collections::BTreeMap;
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
use crate::catalog_delta::CatalogSkill;
use crate::catalog_delta::SkillEntrySnapshot;
use crate::catalog_prompt::SkillPromptKind;

pub(crate) struct PreparedSkillCatalog<'a> {
    pub(super) entries: Vec<&'a SkillCatalogEntry>,
    alias_plan: Option<Cow<'a, AliasPlan>>,
    policy: SkillCatalogRenderPolicy,
}

impl<'a> PreparedSkillCatalog<'a> {
    pub(crate) fn new(catalog: &'a SkillCatalog, policy: SkillCatalogRenderPolicy) -> Self {
        Self::with_alias_plan(catalog, policy, /*alias_plan*/ None)
    }

    pub(crate) fn with_alias_plan(
        catalog: &'a SkillCatalog,
        policy: SkillCatalogRenderPolicy,
        alias_plan: Option<&'a AliasPlan>,
    ) -> Self {
        let mut entries = catalog
            .entries
            .iter()
            .filter(|entry| entry.is_model_visible())
            .collect::<Vec<_>>();
        policy.order_entries(&mut entries);
        let alias_plan = alias_plan
            .map(Cow::Borrowed)
            .or_else(|| build_alias_plan(&entries).map(Cow::Owned));
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

    /// Collect comparison metadata once the final layout and budget have been selected.
    /// Include every eligible entry, even when its rendered row was omitted.
    pub(crate) fn entries_for_render(
        &self,
        rendered: &AvailableSkillsRender,
    ) -> BTreeMap<String, CatalogSkill> {
        let lines = if rendered.skill_root_lines.is_empty() {
            self.unaliased()
        } else {
            self.aliased()
        };
        let shown = rendered
            .included_ids
            .as_ref()
            .map(|ids| ids.iter().collect::<HashSet<_>>());
        lines
            .skills
            .into_iter()
            .map(|line| {
                let id = line.catalog_id();
                let entry = line.entry;
                let fingerprint = blake3::hash(
                    serde_json::json!([
                        entry.name,
                        line.description,
                        entry.main_prompt.as_str(),
                        line.locator,
                    ])
                    .to_string()
                    .as_bytes(),
                )
                .to_hex()
                .to_string();
                let locator = entry.unaliased_locator();
                let root = self
                    .alias_plan
                    .as_ref()
                    .filter(|_| line.locator != locator)
                    .and_then(|plan| plan.root_for_locator(locator))
                    .cloned();
                let shown = shown.as_ref().is_none_or(|ids| ids.contains(&id));
                (
                    id,
                    CatalogSkill {
                        snapshot: SkillEntrySnapshot { fingerprint, shown },
                        line: line.render_full(),
                        root,
                    },
                )
            })
            .collect()
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
            included_ids: selected.included_ids,
            entries: BTreeMap::new(),
            preserve_empty_fragment: self.policy == SkillCatalogRenderPolicy::CoreCompatible,
            report: selected.report,
        })
    }
}
