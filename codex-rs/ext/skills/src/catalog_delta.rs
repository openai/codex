//! Publishes skill additions and edits within one metadata budget per context window.
//! Each snapshot remembers the full inventory and which skills the model has seen.
//! Compaction starts a fresh listing and budget.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use codex_extension_api::ContextualUserFragment;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol::models::input_text;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

use crate::aliases::AliasRoot;
use crate::catalog_prompt::SkillPrompt;
use crate::catalog_prompt::SkillPromptKind;
use crate::fragments::AvailableSkillsInstructions;
use crate::render::AvailableSkillsRender;
use crate::render::SkillMetadataBudget;
use crate::render::SkillRenderReport;
use crate::render::aliased_metadata_overhead_cost;
use crate::render::metadata_line_cost;
use crate::world_state_catalogs::CatalogBudgetAllocation;

const SNAPSHOT_VERSION: u8 = 1;
const ROOTS_HEADING: &str =
    "### Skill roots\nThese mappings replace earlier mappings for the same aliases.\n";
const SKILLS_HEADING: &str = "### Added or updated skills:\n";

#[derive(Deserialize, Serialize)]
pub(crate) struct SkillEntrySnapshot {
    pub(crate) fingerprint: String,
    pub(crate) shown: bool,
}

pub(crate) struct CatalogSkill {
    pub(crate) snapshot: SkillEntrySnapshot,
    pub(crate) line: String,
    pub(crate) root: Option<AliasRoot>,
}

/// Retain both units so switching budget units cannot refund earlier emissions.
#[derive(Clone, Copy, Default, Deserialize, Serialize)]
pub(crate) struct CatalogUsage {
    tokens: usize,
    characters: usize,
}

impl CatalogUsage {
    fn measure(cost: impl Fn(SkillMetadataBudget) -> usize) -> Self {
        Self {
            tokens: cost(SkillMetadataBudget::Tokens(0)),
            characters: cost(SkillMetadataBudget::Characters(0)),
        }
    }

    pub(crate) fn cost(self, budget: SkillMetadataBudget) -> usize {
        match budget {
            SkillMetadataBudget::Tokens(_) => self.tokens,
            SkillMetadataBudget::Characters(_) => self.characters,
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogSnapshot {
    version: u8,
    entries: BTreeMap<String, SkillEntrySnapshot>,
    roots: BTreeMap<String, String>,
    usage: CatalogUsage,
    instructions_sent: bool,
    #[serde(default)]
    omit_descriptions: bool,
    allocation: Option<CatalogBudgetAllocation>,
}

pub(crate) struct CatalogDelta {
    pub(crate) rendered: Option<AvailableSkillsRender>,
    pub(crate) include_usage: bool,
    pub(crate) allocation: Option<CatalogBudgetAllocation>,
}

struct CatalogResult {
    snapshot: CatalogSnapshot,
    body: Option<String>,
    report: SkillRenderReport,
}

pub(crate) fn skills_delta_section(
    id: &'static str,
    source: &'static str,
    catalog: CatalogDelta,
    previous: Option<&serde_json::Value>,
    remaining: &mut SkillMetadataBudget,
    on_render: impl Fn(&SkillRenderReport) + Send + Sync + 'static,
) -> WorldStateSectionContribution {
    // Updates share their remaining budget in source order. A full render, including
    // the one requested by compaction, always starts from the bounded initial listing.
    let update = previous
        .and_then(|value| CatalogSnapshot::deserialize(value).ok())
        .filter(|snapshot| snapshot.version == SNAPSHOT_VERSION)
        .map(|previous| build_catalog_update(source, &catalog, previous, remaining));
    let initial = LazyLock::new(move || build_initial_catalog(source, catalog));
    WorldStateSectionContribution::new(id, move |previous| {
        let result = match previous {
            PreviousWorldStateSection::Known(_) => update.as_ref().unwrap_or_else(|| &initial),
            PreviousWorldStateSection::Absent | PreviousWorldStateSection::Unknown => &initial,
        };
        if result.body.is_some() || result.report.omitted_count > 0 {
            on_render(&result.report);
        }
        let fragment = result.body.as_ref().map(|body| {
            RenderedWorldStateFragment::new(
                "developer",
                (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG),
                input_text(body.clone()),
            )
            .with_metadata(ContentItemMetadata::skills())
        });
        (Some(json!(result.snapshot)), fragment)
    })
    .with_legacy_matcher(|role, text| {
        role == "developer"
            && text.trim_start().starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
            && text.trim_end().ends_with(SKILLS_INSTRUCTIONS_CLOSE_TAG)
    })
}

fn build_initial_catalog(source: &str, catalog: CatalogDelta) -> CatalogResult {
    let mut snapshot = CatalogSnapshot {
        version: SNAPSHOT_VERSION,
        allocation: catalog.allocation,
        ..CatalogSnapshot::default()
    };
    let mut report = SkillRenderReport::default();
    let body = catalog.rendered.and_then(|mut rendered| {
        report = std::mem::take(&mut rendered.report);
        let usage =
            CatalogUsage::measure(|budget| rendered.metadata_cost(budget, catalog.include_usage));
        let entries = std::mem::take(&mut rendered.entries);
        snapshot.omit_descriptions = rendered.prompt.omit_descriptions;
        let fragment = rendered.into_fragment(catalog.include_usage);
        for (id, skill) in entries {
            if fragment.is_some()
                && let Some(root) = skill.root
            {
                snapshot.roots.insert(root.name, root.value.to_string());
            }
            snapshot.entries.insert(id, skill.snapshot);
        }
        let fragment = fragment?;
        snapshot.instructions_sent = true;
        snapshot.usage = usage;
        Some(listing_body(source, fragment))
    });
    CatalogResult {
        snapshot,
        report,
        body,
    }
}

fn build_catalog_update(
    source: &str,
    catalog: &CatalogDelta,
    mut previous: CatalogSnapshot,
    remaining: &mut SkillMetadataBudget,
) -> CatalogResult {
    let usage_update = catalog.rendered.as_ref().filter(|rendered| {
        previous.instructions_sent
            && catalog.include_usage
            && previous.omit_descriptions != rendered.prompt.omit_descriptions
    });
    let message = if previous.instructions_sent {
        CatalogMessage::Update {
            heading: format!("\n## {source} skills update\n"),
        }
    } else {
        CatalogMessage::FirstListing {
            prompt: catalog.rendered.as_ref().map_or(
                SkillPrompt {
                    kind: SkillPromptKind::Unaliased,
                    omit_descriptions: false,
                },
                |r| r.prompt,
            ),
            include_usage: catalog.include_usage,
        }
    };
    let mut draft = UpdateDraft {
        message,
        skill_lines: Vec::new(),
        root_lines: Vec::new(),
    };
    let mut report = SkillRenderReport::default();
    let mut entries = BTreeMap::new();
    for (id, skill) in catalog.rendered.iter().flat_map(|r| &r.entries) {
        let old = previous.entries.get(id);
        let mut shown = old.is_some_and(|old| old.shown);
        if old.is_none_or(|old| old.shown && old.fingerprint != skill.snapshot.fingerprint) {
            report.total_count += 1;
            if draft.try_append_skill(skill, &mut previous, remaining) {
                shown = true;
                report.included_count += 1;
            } else {
                report.omitted_count += 1;
            }
        }
        entries.insert(
            id.clone(),
            SkillEntrySnapshot {
                fingerprint: skill.snapshot.fingerprint.clone(),
                shown,
            },
        );
    }
    let mut body = draft.into_body(source);
    if let Some(rendered) = usage_update {
        let instructions = rendered.prompt.usage_instructions();
        body.get_or_insert_with(|| format!("\n## {source} skills update\n"))
            .push_str(&format!("\n### How to use skills\n{instructions}\n"));
    }
    if let Some(rendered) = &catalog.rendered {
        previous.omit_descriptions = rendered.prompt.omit_descriptions;
    }
    previous.entries = entries;
    previous.instructions_sent |= body.is_some();
    previous.allocation = catalog.allocation;
    CatalogResult {
        snapshot: previous,
        report,
        body,
    }
}

fn listing_body(source: &str, instructions: AvailableSkillsInstructions) -> String {
    format!(
        "\n## {source} skills\n{}",
        instructions.body().into_text().unwrap_or_default()
    )
}

enum CatalogMessage {
    FirstListing {
        prompt: SkillPrompt,
        include_usage: bool,
    },
    Update {
        heading: String,
    },
}

struct UpdateDraft {
    message: CatalogMessage,
    skill_lines: Vec<String>,
    root_lines: Vec<String>,
}

impl UpdateDraft {
    fn try_append_skill(
        &mut self,
        skill: &CatalogSkill,
        snapshot: &mut CatalogSnapshot,
        remaining: &mut SkillMetadataBudget,
    ) -> bool {
        let new_mapping = skill
            .root
            .as_ref()
            .filter(|root| {
                snapshot.roots.get(&root.name).map(String::as_str) != Some(root.value.as_ref())
            })
            .map(|root| (root, root.line()));
        let cost = CatalogUsage::measure(|budget| {
            let mut cost = metadata_line_cost(budget, &skill.line);
            if let CatalogMessage::Update { heading } = &self.message
                && self.skill_lines.is_empty()
            {
                cost += budget.cost(heading) + budget.cost(SKILLS_HEADING);
            }
            if let Some((_, line)) = &new_mapping {
                cost += match &self.message {
                    CatalogMessage::FirstListing {
                        prompt,
                        include_usage,
                    } if self.root_lines.is_empty() => aliased_metadata_overhead_cost(
                        budget,
                        *prompt,
                        std::slice::from_ref(line),
                        *include_usage,
                    ),
                    CatalogMessage::Update { .. } if self.root_lines.is_empty() => {
                        budget.cost(ROOTS_HEADING) + metadata_line_cost(budget, line)
                    }
                    CatalogMessage::FirstListing { .. } | CatalogMessage::Update { .. } => {
                        metadata_line_cost(budget, line)
                    }
                };
            }
            cost
        });
        let charge = cost.cost(*remaining);
        if charge > remaining.limit() {
            return false;
        }
        *remaining = remaining.with_limit(remaining.limit() - charge);
        snapshot.usage.tokens += cost.tokens;
        snapshot.usage.characters += cost.characters;
        if let Some((root, line)) = new_mapping {
            snapshot
                .roots
                .insert(root.name.clone(), root.value.to_string());
            self.root_lines.push(line);
        }
        self.skill_lines.push(skill.line.clone());
        true
    }

    fn into_body(self, source: &str) -> Option<String> {
        if self.skill_lines.is_empty() {
            return None;
        }
        Some(match self.message {
            CatalogMessage::FirstListing {
                mut prompt,
                include_usage,
            } => {
                if self.root_lines.is_empty() {
                    prompt.kind = SkillPromptKind::Unaliased;
                }
                listing_body(
                    source,
                    AvailableSkillsInstructions::from_skill_lines(
                        prompt,
                        self.root_lines,
                        self.skill_lines,
                        include_usage,
                    ),
                )
            }
            CatalogMessage::Update { mut heading } => {
                if !self.root_lines.is_empty() {
                    heading.push_str(ROOTS_HEADING);
                    heading.push_str(&self.root_lines.join("\n"));
                    heading.push('\n');
                }
                heading.push_str(SKILLS_HEADING);
                heading.push_str(&self.skill_lines.join("\n"));
                heading.push('\n');
                heading
            }
        })
    }
}

#[cfg(test)]
#[path = "catalog_delta_tests.rs"]
mod tests;
