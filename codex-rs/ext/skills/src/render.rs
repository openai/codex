use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_core_skills::render_available_skills_body;
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
const SKILL_DESCRIPTION_TRUNCATION_WARNING_THRESHOLD_CHARS: usize = 100;
const APPROX_BYTES_PER_TOKEN: usize = 4;
const SKILL_DESCRIPTION_TRUNCATED_WARNING: &str = "Skill descriptions were shortened to fit the skills context budget. Codex can still see every skill, but some descriptions are shorter. Disable unused skills or plugins to leave more room for the rest.";
const SKILL_DESCRIPTIONS_REMOVED_WARNING_PREFIX: &str =
    "Exceeded skills context budget. All skill descriptions were removed and";
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

    fn includes_omission_notice(self) -> bool {
        match self {
            Self::CoreCompatible => false,
            Self::ExtensionCompatible => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillMetadataBudget {
    Tokens(usize),
    Characters(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillRenderReport {
    pub(crate) total_count: usize,
    pub(crate) included_count: usize,
    pub(crate) omitted_count: usize,
    pub(crate) truncated_description_chars: usize,
    pub(crate) truncated_description_count: usize,
}

impl SkillRenderReport {
    pub(crate) fn warning_message(&self) -> Option<String> {
        if self.omitted_count > 0 {
            let skill_word = if self.omitted_count == 1 {
                "skill"
            } else {
                "skills"
            };
            let verb = if self.omitted_count == 1 {
                "was"
            } else {
                "were"
            };
            return Some(format!(
                "{} {} additional {} {} not included in the model-visible skills list.",
                SKILL_DESCRIPTIONS_REMOVED_WARNING_PREFIX, self.omitted_count, skill_word, verb
            ));
        }

        (self.average_truncated_description_chars()
            > SKILL_DESCRIPTION_TRUNCATION_WARNING_THRESHOLD_CHARS)
            .then(|| SKILL_DESCRIPTION_TRUNCATED_WARNING.to_string())
    }

    fn average_truncated_description_chars(&self) -> usize {
        if self.total_count == 0 || self.truncated_description_chars == 0 {
            return 0;
        }

        self.truncated_description_chars
            .saturating_add(self.total_count.saturating_sub(1))
            / self.total_count
    }
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

    fn cost(self, text: &str) -> usize {
        match self {
            Self::Tokens(_) => approx_token_count(text),
            Self::Characters(_) => text.chars().count(),
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
        Self::with_locator(entry, policy, entry.rendered_path().to_string())
    }

    fn with_locator(
        entry: &'a SkillCatalogEntry,
        policy: SkillCatalogRenderPolicy,
        locator: String,
    ) -> Self {
        let description = policy.description(entry);
        Self {
            name: entry.name.as_str(),
            description: truncate_catalog_skill_description(description),
            locator,
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

    fn description_char_count(&self) -> usize {
        self.description.chars().count()
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

struct RenderedSkillLine {
    line: String,
    truncated_description_chars: usize,
}

struct RenderedSkillLines {
    lines: Vec<RenderedSkillLine>,
    omitted_count: usize,
    truncated_description_chars: usize,
    truncated_description_count: usize,
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
) -> RenderedSkillLines {
    let full_cost = skill_lines.iter().fold(0usize, |used, line| {
        used.saturating_add(line.full_cost(budget))
    });
    if full_cost <= budget.limit() {
        return RenderedSkillLines {
            lines: skill_lines
                .iter()
                .map(|line| RenderedSkillLine {
                    line: line.render_full(),
                    truncated_description_chars: 0,
                })
                .collect(),
            omitted_count: 0,
            truncated_description_chars: 0,
            truncated_description_count: 0,
        };
    }

    let minimum_cost = skill_lines.iter().fold(0usize, |used, line| {
        used.saturating_add(line.minimum_cost(budget))
    });
    if minimum_cost <= budget.limit() {
        let lines = render_lines_with_description_budget(
            budget,
            &skill_lines,
            budget.limit().saturating_sub(minimum_cost),
        );
        let (truncated_description_chars, truncated_description_count) =
            sum_description_truncation(&lines);
        return RenderedSkillLines {
            lines,
            omitted_count: 0,
            truncated_description_chars,
            truncated_description_count,
        };
    }

    let mut included = Vec::new();
    let mut used = 0usize;
    let mut omitted = 0usize;
    let mut truncated_description_chars = 0usize;
    let mut truncated_description_count = 0usize;
    for line in skill_lines {
        let description_char_count = line.description_char_count();
        let rendered = line.render_minimum();
        let next_used = used.saturating_add(line.minimum_cost(budget));
        if next_used <= budget.limit() {
            used = next_used;
            included.push(RenderedSkillLine {
                line: rendered,
                truncated_description_chars: description_char_count,
            });
        } else {
            omitted = omitted.saturating_add(1);
        }

        truncated_description_chars =
            truncated_description_chars.saturating_add(description_char_count);
        if description_char_count > 0 {
            truncated_description_count = truncated_description_count.saturating_add(1);
        }
    }
    RenderedSkillLines {
        lines: included,
        omitted_count: omitted,
        truncated_description_chars,
        truncated_description_count,
    }
}

fn render_lines_with_description_budget(
    budget: SkillMetadataBudget,
    skill_lines: &[SkillLine<'_>],
    limit: usize,
) -> Vec<RenderedSkillLine> {
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
        .map(|(line, description_chars)| RenderedSkillLine {
            line: line.line.render_with_description_chars(description_chars),
            truncated_description_chars: line
                .description_char_count
                .saturating_sub(description_chars),
        })
        .collect()
}

fn sum_description_truncation(rendered: &[RenderedSkillLine]) -> (usize, usize) {
    rendered
        .iter()
        .fold((0usize, 0usize), |(chars, count), line| {
            if line.truncated_description_chars == 0 {
                (chars, count)
            } else {
                (
                    chars.saturating_add(line.truncated_description_chars),
                    count.saturating_add(1),
                )
            }
        })
}

struct RenderedCatalog {
    skill_root_lines: Vec<String>,
    skill_lines: Vec<String>,
    report: SkillRenderReport,
}

pub(crate) struct AvailableSkillsRender {
    skill_root_lines: Vec<String>,
    skill_lines: Vec<String>,
    pub(crate) report: SkillRenderReport,
}

impl AvailableSkillsRender {
    pub(crate) fn into_fragment(
        self,
        include_skills_usage_instructions: bool,
    ) -> Option<AvailableSkillsInstructions> {
        (!self.skill_lines.is_empty()).then(|| {
            AvailableSkillsInstructions::from_skill_lines(
                self.skill_root_lines,
                self.skill_lines,
                include_skills_usage_instructions,
            )
        })
    }
}

#[tracing::instrument(
    level = "trace",
    skip_all,
    fields(catalog_entry_count = catalog.entries.len())
)]
pub(crate) fn render_available_skills(
    catalog: &SkillCatalog,
    policy: SkillCatalogRenderPolicy,
    budget: SkillMetadataBudget,
) -> Option<AvailableSkillsRender> {
    let mut entries = catalog
        .entries
        .iter()
        .filter(|entry| entry.enabled && entry.prompt_visible)
        .collect::<Vec<_>>();
    policy.order_entries(&mut entries);
    if entries.is_empty() {
        return None;
    }

    let absolute = render_catalog(
        entries
            .iter()
            .map(|entry| SkillLine::new(entry, policy))
            .collect(),
        budget,
        Vec::new(),
        policy,
    );
    let selected =
        if absolute.report.omitted_count == 0 && absolute.report.truncated_description_chars == 0 {
            absolute
        } else if let Some(aliased) = build_aliased_catalog(&entries, policy, budget)
            && aliased_render_is_better(&aliased, &absolute, budget)
        {
            aliased
        } else {
            absolute
        };

    Some(AvailableSkillsRender {
        skill_root_lines: selected.skill_root_lines,
        skill_lines: selected.skill_lines,
        report: selected.report,
    })
}

fn render_catalog(
    skill_lines: Vec<SkillLine<'_>>,
    budget: SkillMetadataBudget,
    skill_root_lines: Vec<String>,
    policy: SkillCatalogRenderPolicy,
) -> RenderedCatalog {
    let total_count = skill_lines.len();
    let RenderedSkillLines {
        lines: mut rendered_lines,
        omitted_count: mut omitted,
        truncated_description_chars,
        truncated_description_count,
    } = render_skill_lines(skill_lines, budget);
    let mut total_cost = rendered_lines.iter().fold(0usize, |used, rendered| {
        used.saturating_add(metadata_line_cost(budget, &rendered.line))
    });

    if omitted > 0 && policy.includes_omission_notice() {
        loop {
            let marker = omission_marker(omitted);
            if total_cost.saturating_add(metadata_line_cost(budget, &marker)) <= budget.limit() {
                rendered_lines.push(RenderedSkillLine {
                    line: marker,
                    truncated_description_chars: 0,
                });
                break;
            }
            let Some(rendered) = rendered_lines.pop() else {
                break;
            };
            total_cost = total_cost.saturating_sub(metadata_line_cost(budget, &rendered.line));
            omitted = omitted.saturating_add(1);
        }
    }

    RenderedCatalog {
        skill_root_lines,
        skill_lines: rendered_lines
            .into_iter()
            .map(|rendered| rendered.line)
            .collect(),
        report: SkillRenderReport {
            total_count,
            included_count: total_count.saturating_sub(omitted),
            omitted_count: omitted,
            truncated_description_chars,
            truncated_description_count,
        },
    }
}

#[cfg(test)]
fn available_skills_fragment(
    catalog: &SkillCatalog,
    include_skills_usage_instructions: bool,
    policy: SkillCatalogRenderPolicy,
    budget: SkillMetadataBudget,
) -> Option<AvailableSkillsInstructions> {
    render_available_skills(catalog, policy, budget)?
        .into_fragment(include_skills_usage_instructions)
}

struct AliasPlan {
    skill_root_lines: Vec<String>,
    alias_root_by_display_root: HashMap<String, String>,
    root_aliases: HashMap<String, String>,
    table_cost: usize,
}

fn build_aliased_catalog(
    entries: &[&SkillCatalogEntry],
    policy: SkillCatalogRenderPolicy,
    budget: SkillMetadataBudget,
) -> Option<RenderedCatalog> {
    let plan = build_alias_plan(entries, budget)?;
    if plan.table_cost >= budget.limit() {
        return None;
    }

    let adjusted_limit = budget.limit().saturating_sub(plan.table_cost);
    let adjusted_budget = match budget {
        SkillMetadataBudget::Tokens(_) => SkillMetadataBudget::Tokens(adjusted_limit),
        SkillMetadataBudget::Characters(_) => SkillMetadataBudget::Characters(adjusted_limit),
    };
    let skill_lines = entries
        .iter()
        .map(|entry| {
            SkillLine::with_locator(entry, policy, render_skill_path_with_aliases(entry, &plan))
        })
        .collect();
    Some(render_catalog(
        skill_lines,
        adjusted_budget,
        plan.skill_root_lines,
        policy,
    ))
}

fn build_alias_plan(
    entries: &[&SkillCatalogEntry],
    budget: SkillMetadataBudget,
) -> Option<AliasPlan> {
    // The shared alias prompt only describes host filesystem skills.
    if entries
        .iter()
        .any(|entry| entry.authority.kind != SkillSourceKind::Host)
    {
        return None;
    }

    let plugin_version_skill_counts = plugin_version_skill_counts_for_entries(entries);
    let mut alias_root_by_display_root = HashMap::new();
    let mut alias_roots = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        if entry.authority.kind != SkillSourceKind::Host {
            continue;
        }
        let Some(display_root) = entry.display_path_root() else {
            continue;
        };
        let alias_root =
            alias_root_for_display_root(Path::new(display_root), &plugin_version_skill_counts)
                .to_string_lossy()
                .replace('\\', "/");
        alias_root_by_display_root.insert(display_root.to_string(), alias_root.clone());
        if seen.insert(alias_root.clone()) {
            alias_roots.push(alias_root);
        }
    }
    if alias_roots.is_empty() {
        return None;
    }

    let root_aliases = alias_roots
        .iter()
        .enumerate()
        .map(|(index, root)| (root.clone(), format!("r{index}")))
        .collect();
    let skill_root_lines = alias_roots
        .iter()
        .enumerate()
        .map(|(index, root)| format!("- `r{index}` = `{root}`"))
        .collect::<Vec<_>>();
    let table_cost = aliased_metadata_overhead_cost(budget, &skill_root_lines);
    Some(AliasPlan {
        skill_root_lines,
        alias_root_by_display_root,
        root_aliases,
        table_cost,
    })
}

fn plugin_version_skill_counts_for_entries(
    entries: &[&SkillCatalogEntry],
) -> HashMap<PathBuf, usize> {
    let mut counts = HashMap::new();
    for root in entries.iter().filter_map(|entry| {
        (entry.authority.kind == SkillSourceKind::Host)
            .then(|| entry.display_path_root())
            .flatten()
    }) {
        if let Some(plugin_version_base) = plugin_version_base(Path::new(root)) {
            let count = counts.entry(plugin_version_base).or_insert(0usize);
            *count = count.saturating_add(1);
        }
    }
    counts
}

fn alias_root_for_display_root(
    root: &Path,
    plugin_version_skill_counts: &HashMap<PathBuf, usize>,
) -> PathBuf {
    let Some(plugin_version_base) = plugin_version_base(root) else {
        return root.to_path_buf();
    };
    let skill_count = plugin_version_skill_counts
        .get(&plugin_version_base)
        .copied()
        .unwrap_or_default();
    if skill_count > 1 {
        root.to_path_buf()
    } else {
        plugin_marketplace_base(root).unwrap_or_else(|| root.to_path_buf())
    }
}

fn plugin_marketplace_base(path: &Path) -> Option<PathBuf> {
    let mut candidate = path;
    while let Some(parent) = candidate.parent() {
        if parent.file_name()?.to_str()? == "cache"
            && parent.parent()?.file_name()?.to_str()? == "plugins"
        {
            return Some(candidate.to_path_buf());
        }
        candidate = parent;
    }
    None
}

fn plugin_version_base(path: &Path) -> Option<PathBuf> {
    let marketplace_base = plugin_marketplace_base(path)?;
    let mut relative_components = path.strip_prefix(&marketplace_base).ok()?.components();
    let plugin = match relative_components.next()? {
        Component::Normal(plugin) => plugin,
        _ => return None,
    };
    let version = match relative_components.next()? {
        Component::Normal(version) => version,
        _ => return None,
    };
    Some(marketplace_base.join(plugin).join(version))
}

fn render_skill_path_with_aliases(entry: &SkillCatalogEntry, plan: &AliasPlan) -> String {
    if entry.authority.kind != SkillSourceKind::Host {
        return entry.rendered_path().to_string();
    }
    let Some(display_root) = entry.display_path_root() else {
        return entry.rendered_path().to_string();
    };
    let Some(alias_root) = plan.alias_root_by_display_root.get(display_root) else {
        return entry.rendered_path().to_string();
    };
    let Some(alias) = plan.root_aliases.get(alias_root) else {
        return entry.rendered_path().to_string();
    };
    let Ok(relative_path) = Path::new(entry.rendered_path()).strip_prefix(alias_root) else {
        return entry.rendered_path().to_string();
    };
    let relative_path = relative_path.to_string_lossy().replace('\\', "/");
    format!("{alias}/{relative_path}")
}

fn aliased_metadata_overhead_cost(
    budget: SkillMetadataBudget,
    skill_root_lines: &[String],
) -> usize {
    let empty_skill_lines: &[String] = &[];
    let absolute_body = render_available_skills_body(&[], empty_skill_lines);
    let aliased_body = render_available_skills_body(skill_root_lines, empty_skill_lines);
    budget
        .cost(&aliased_body)
        .saturating_sub(budget.cost(&absolute_body))
}

fn aliased_render_is_better(
    aliased: &RenderedCatalog,
    absolute: &RenderedCatalog,
    budget: SkillMetadataBudget,
) -> bool {
    if aliased.report.included_count != absolute.report.included_count {
        return aliased.report.included_count > absolute.report.included_count;
    }
    if aliased.report.truncated_description_chars != absolute.report.truncated_description_chars {
        return aliased.report.truncated_description_chars
            < absolute.report.truncated_description_chars;
    }
    rendered_catalog_cost(budget, aliased) < rendered_catalog_cost(budget, absolute)
}

fn rendered_catalog_cost(budget: SkillMetadataBudget, rendered: &RenderedCatalog) -> usize {
    let metadata_cost = if rendered.skill_root_lines.is_empty() {
        0
    } else {
        aliased_metadata_overhead_cost(budget, &rendered.skill_root_lines)
    };
    rendered
        .skill_lines
        .iter()
        .fold(metadata_cost, |used, line| {
            used.saturating_add(metadata_line_cost(budget, line))
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
