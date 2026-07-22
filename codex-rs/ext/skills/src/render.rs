use std::borrow::Cow;

use codex_protocol::protocol::SkillScope;
use codex_utils_string::approx_token_count;
use codex_utils_string::take_bytes_at_char_boundary;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillSourceKind;
use crate::fragments::AvailableSkillsInstructions;

const DEFAULT_SKILL_METADATA_CHAR_BUDGET: usize = 8_000;
const MAX_SKILL_METADATA_TOKEN_BUDGET: usize = 4_000;
const SKILL_METADATA_CONTEXT_WINDOW_PERCENT: usize = 2;
const MAX_MAIN_PROMPT_BYTES: usize = 8_000;
const MAX_CATALOG_SKILL_DESCRIPTION_CHARS: usize = 1_024;
const TRUNCATED_SKILL_DESCRIPTION_SUFFIX: &str = "...";
const APPROX_BYTES_PER_TOKEN: usize = 4;
pub(crate) const MAX_SKILL_NAME_BYTES: usize = 256;
pub(crate) const MAX_SKILL_PATH_BYTES: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillCatalogRenderPolicy {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "used by the host renderer compatibility path in a follow-up"
        )
    )]
    CoreCompatible,
    ExtensionCompatible,
}

impl SkillCatalogRenderPolicy {
    fn description(self, entry: &SkillCatalogEntry) -> &str {
        match self {
            Self::CoreCompatible => entry.description.as_str(),
            Self::ExtensionCompatible => entry
                .short_description
                .as_deref()
                .unwrap_or(entry.description.as_str()),
        }
    }

    fn order_entries(self, entries: &mut [&SkillCatalogEntry]) {
        match self {
            Self::CoreCompatible => {
                let scope_rank = |entry: &SkillCatalogEntry| match entry.prompt_scope() {
                    Some(SkillScope::System) => 0,
                    Some(SkillScope::Admin) => 1,
                    Some(SkillScope::Repo) => 2,
                    Some(SkillScope::User) => 3,
                    None => 4,
                };
                entries.sort_by(|a, b| {
                    scope_rank(a)
                        .cmp(&scope_rank(b))
                        .then_with(|| a.name.cmp(&b.name))
                        .then_with(|| a.main_prompt.as_str().cmp(b.main_prompt.as_str()))
                });
            }
            Self::ExtensionCompatible => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillMetadataBudget {
    Tokens(usize),
    Characters(usize),
}

pub(crate) fn capped_skill_metadata_budget(context_window: Option<i64>) -> SkillMetadataBudget {
    context_window
        .and_then(|window| usize::try_from(window).ok())
        .filter(|window| *window > 0)
        .map(|window| {
            SkillMetadataBudget::Tokens(
                window
                    .saturating_mul(SKILL_METADATA_CONTEXT_WINDOW_PERCENT)
                    .saturating_div(100)
                    .clamp(1, MAX_SKILL_METADATA_TOKEN_BUDGET),
            )
        })
        .unwrap_or(SkillMetadataBudget::Characters(
            DEFAULT_SKILL_METADATA_CHAR_BUDGET,
        ))
}

fn metadata_line_cost(budget: SkillMetadataBudget, line: &str) -> usize {
    let line = format!("{line}\n");
    match budget {
        SkillMetadataBudget::Tokens(_) => approx_token_count(&line),
        SkillMetadataBudget::Characters(_) => line.chars().count(),
    }
}

impl SkillMetadataBudget {
    fn limit(self) -> usize {
        match self {
            Self::Tokens(limit) | Self::Characters(limit) => limit,
        }
    }

    fn cost_from_counts(self, chars: usize, bytes: usize) -> usize {
        match self {
            Self::Tokens(_) => {
                bytes.saturating_add(APPROX_BYTES_PER_TOKEN.saturating_sub(1))
                    / APPROX_BYTES_PER_TOKEN
            }
            Self::Characters(_) => chars,
        }
    }
}

struct SkillLine<'a> {
    name: &'a str,
    description: Cow<'a, str>,
    locator: String,
    locator_kind: &'static str,
}

impl<'a> SkillLine<'a> {
    fn new(entry: &'a SkillCatalogEntry, policy: SkillCatalogRenderPolicy) -> Self {
        let description = policy.description(entry);
        Self {
            name: entry.name.as_str(),
            description: truncate_catalog_skill_description(description),
            locator: entry.rendered_path().to_string(),
            locator_kind: match &entry.authority.kind {
                SkillSourceKind::Host => "file",
                SkillSourceKind::Executor => "environment resource",
                SkillSourceKind::Orchestrator => "orchestrator resource",
                SkillSourceKind::Custom(_) => "custom resource",
            },
        }
    }

    fn full_cost(&self, budget: SkillMetadataBudget) -> usize {
        metadata_line_cost(budget, &self.render_full())
    }

    fn minimum_cost(&self, budget: SkillMetadataBudget) -> usize {
        metadata_line_cost(budget, &self.render_minimum())
    }

    fn render_full(&self) -> String {
        self.render_with_description(self.description.as_ref())
    }

    fn render_minimum(&self) -> String {
        self.render_with_description("")
    }

    fn render_with_description_chars(&self, description_chars: usize) -> String {
        let end = self
            .description
            .char_indices()
            .nth(description_chars)
            .map_or(self.description.len(), |(index, _)| index);
        self.render_with_description(&self.description[..end])
    }

    fn render_with_description(&self, description: &str) -> String {
        let name = self.name;
        let locator = self.locator.as_str();
        let locator_kind = self.locator_kind;
        if description.is_empty() {
            format!("- {name}: ({locator_kind}: {locator})")
        } else {
            format!("- {name}: {description} ({locator_kind}: {locator})")
        }
    }
}

struct DescriptionBudgetLine<'a> {
    line: &'a SkillLine<'a>,
    description_char_count: usize,
    extra_costs: Vec<usize>,
}

impl<'a> DescriptionBudgetLine<'a> {
    fn new(line: &'a SkillLine<'a>, budget: SkillMetadataBudget) -> Self {
        let minimum_line = line.render_minimum();
        let minimum_chars = minimum_line.chars().count().saturating_add(1);
        let minimum_bytes = minimum_line.len().saturating_add(1);
        let minimum_cost = budget.cost_from_counts(minimum_chars, minimum_bytes);

        let description_char_count = line.description.chars().count();
        let mut extra_costs = Vec::with_capacity(description_char_count.saturating_add(1));
        extra_costs.push(0);

        let mut prefix_chars = 0usize;
        let mut prefix_bytes = 0usize;
        for ch in line.description.chars() {
            prefix_chars = prefix_chars.saturating_add(1);
            prefix_bytes = prefix_bytes.saturating_add(ch.len_utf8());
            let rendered_chars = minimum_chars.saturating_add(prefix_chars).saturating_add(1);
            let rendered_bytes = minimum_bytes.saturating_add(prefix_bytes).saturating_add(1);
            let cost = budget
                .cost_from_counts(rendered_chars, rendered_bytes)
                .saturating_sub(minimum_cost);
            extra_costs.push(cost);
        }

        Self {
            line,
            description_char_count,
            extra_costs,
        }
    }
}

fn render_skill_lines(
    skill_lines: Vec<SkillLine<'_>>,
    budget: SkillMetadataBudget,
) -> (Vec<String>, usize) {
    let full_cost = skill_lines.iter().fold(0usize, |used, line| {
        used.saturating_add(line.full_cost(budget))
    });
    if full_cost <= budget.limit() {
        return (skill_lines.iter().map(SkillLine::render_full).collect(), 0);
    }

    let minimum_cost = skill_lines.iter().fold(0usize, |used, line| {
        used.saturating_add(line.minimum_cost(budget))
    });
    if minimum_cost <= budget.limit() {
        return (
            render_lines_with_description_budget(
                budget,
                &skill_lines,
                budget.limit().saturating_sub(minimum_cost),
            ),
            0,
        );
    }

    let mut included = Vec::new();
    let mut used = 0usize;
    let mut omitted = 0usize;
    for line in skill_lines {
        let rendered = line.render_minimum();
        let next_used = used.saturating_add(line.minimum_cost(budget));
        if next_used <= budget.limit() {
            used = next_used;
            included.push(rendered);
        } else {
            omitted = omitted.saturating_add(1);
        }
    }
    (included, omitted)
}

fn render_lines_with_description_budget(
    budget: SkillMetadataBudget,
    skill_lines: &[SkillLine<'_>],
    limit: usize,
) -> Vec<String> {
    let budget_lines = skill_lines
        .iter()
        .map(|line| DescriptionBudgetLine::new(line, budget))
        .collect::<Vec<_>>();
    let mut char_allocations = vec![0usize; budget_lines.len()];
    let mut current_extra_costs = vec![0usize; budget_lines.len()];
    let mut remaining = limit;

    // Distribute description space round-robin so no skill monopolizes the
    // remaining budget.
    loop {
        let mut changed = false;
        for (index, line) in budget_lines.iter().enumerate() {
            if char_allocations[index] >= line.description_char_count {
                continue;
            }

            let next_chars = char_allocations[index].saturating_add(1);
            let next_cost = line.extra_costs[next_chars];
            let delta = next_cost.saturating_sub(current_extra_costs[index]);
            if delta <= remaining {
                char_allocations[index] = next_chars;
                current_extra_costs[index] = next_cost;
                remaining = remaining.saturating_sub(delta);
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    budget_lines
        .iter()
        .zip(char_allocations)
        .map(|(line, description_chars)| line.line.render_with_description_chars(description_chars))
        .collect()
}

#[tracing::instrument(
    level = "trace",
    skip_all,
    fields(catalog_entry_count = catalog.entries.len())
)]
pub(crate) fn available_skills_fragment(
    catalog: &SkillCatalog,
    include_skills_usage_instructions: bool,
    policy: SkillCatalogRenderPolicy,
    budget: SkillMetadataBudget,
) -> Option<AvailableSkillsInstructions> {
    let mut entries = catalog
        .entries
        .iter()
        .filter(|entry| entry.enabled && entry.prompt_visible)
        .collect::<Vec<_>>();
    policy.order_entries(&mut entries);
    let skill_lines = entries
        .iter()
        .map(|entry| SkillLine::new(entry, policy))
        .collect();
    let (mut skill_lines, mut omitted) = render_skill_lines(skill_lines, budget);
    let mut total_cost = skill_lines.iter().fold(0usize, |used, line| {
        used.saturating_add(metadata_line_cost(budget, line))
    });

    if omitted > 0 {
        loop {
            let marker = omission_marker(omitted);
            if total_cost.saturating_add(metadata_line_cost(budget, &marker)) <= budget.limit() {
                skill_lines.push(marker);
                break;
            }
            let line = skill_lines.pop()?;
            total_cost = total_cost.saturating_sub(metadata_line_cost(budget, &line));
            omitted = omitted.saturating_add(1);
        }
    }

    (!skill_lines.is_empty()).then(|| {
        AvailableSkillsInstructions::from_skill_lines(
            skill_lines,
            include_skills_usage_instructions,
        )
    })
}

fn omission_marker(omitted: usize) -> String {
    let skill_word = if omitted == 1 { "skill" } else { "skills" };
    format!("- {omitted} additional {skill_word} omitted from this bounded skills list.")
}

pub(crate) fn truncate_catalog_skill_description(description: &str) -> Cow<'_, str> {
    if description
        .char_indices()
        .nth(MAX_CATALOG_SKILL_DESCRIPTION_CHARS)
        .is_none()
    {
        return Cow::Borrowed(description);
    }

    let prefix_chars = MAX_CATALOG_SKILL_DESCRIPTION_CHARS
        .saturating_sub(TRUNCATED_SKILL_DESCRIPTION_SUFFIX.chars().count());
    let prefix_end = description
        .char_indices()
        .nth(prefix_chars)
        .map_or(description.len(), |(index, _)| index);
    let mut truncated = description[..prefix_end].to_string();
    truncated.push_str(TRUNCATED_SKILL_DESCRIPTION_SUFFIX);
    Cow::Owned(truncated)
}

pub(crate) fn truncate_main_prompt_contents(contents: &str) -> (String, bool) {
    truncate_utf8_to_bytes(contents, MAX_MAIN_PROMPT_BYTES)
}

pub(crate) fn truncate_utf8_to_bytes(contents: &str, max_bytes: usize) -> (String, bool) {
    let truncated = take_bytes_at_char_boundary(contents, max_bytes);
    (truncated.to_string(), truncated.len() < contents.len())
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
