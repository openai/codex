use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::model::SkillLoadOutcome;
use crate::model::SkillMetadata;
use codex_otel::SessionTelemetry;
use codex_otel::THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC;
use codex_otel::THREAD_SKILLS_ENABLED_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_KEPT_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_TRUNCATED_METRIC;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_output_truncation::approx_token_count;

const DEFAULT_SKILL_METADATA_CHAR_BUDGET: usize = 8_000;
const SKILL_METADATA_CONTEXT_WINDOW_PERCENT: usize = 2;
const SKILL_DESCRIPTION_TRUNCATION_WARNING_THRESHOLD_CHARS: usize = 10;
const APPROX_BYTES_PER_TOKEN: usize = 4;
pub const SKILL_DESCRIPTION_TRUNCATED_WARNING_PREFIX: &str = "Warning: Exceeded skills context budget. Loaded skill descriptions were truncated by an average of";
pub const SKILL_DESCRIPTIONS_REMOVED_WARNING_PREFIX: &str =
    "Warning: Exceeded skills context budget. All skill descriptions were removed and";
pub const SKILLS_INTRO_WITH_ABSOLUTE_PATHS: &str = "A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and file path so you can open the source for full instructions when using a specific skill.";
pub const SKILLS_INTRO_WITH_ALIASES: &str = "A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and a short path that can be expanded into an absolute path using the skill roots table.";
pub const SKILLS_HOW_TO_USE_WITH_ABSOLUTE_PATHS: &str = r###"- Discovery: The list above is the skills available in this session (name + description + file path). Skill bodies live on disk at the listed paths.
- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.
- How to use a skill (progressive disclosure):
  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.
  2) When `SKILL.md` references relative paths (e.g., `scripts/foo.py`), resolve them relative to the skill directory listed above first, and only consider other paths if needed.
  3) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.
  4) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.
  5) If `assets/` or templates exist, reuse them instead of recreating from scratch.
- Coordination and sequencing:
  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.
  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.
- Context hygiene:
  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.
  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.
  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.
- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue."###;
pub const SKILLS_HOW_TO_USE_WITH_ALIASES: &str = r###"- Discovery: The list above is the skills available in this session (name + description + short path). Skill bodies live on disk at the listed paths after expanding the matching alias from `### Skill roots`.
- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.
- How to use a skill (progressive disclosure):
  1) After deciding to use a skill, expand the listed short `path` with the matching alias from `### Skill roots`, then open its `SKILL.md`. Read only enough to follow the workflow.
  2) When `SKILL.md` references relative paths (e.g., `scripts/foo.py`), resolve them relative to the directory containing that expanded `SKILL.md` first, and only consider other paths if needed.
  3) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.
  4) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.
  5) If `assets/` or templates exist, reuse them instead of recreating from scratch.
- Coordination and sequencing:
  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.
  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.
- Context hygiene:
  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.
  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.
  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.
- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue."###;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillMetadataBudget {
    Tokens(usize),
    Characters(usize),
}

impl SkillMetadataBudget {
    fn limit(self) -> usize {
        match self {
            Self::Tokens(limit) | Self::Characters(limit) => limit,
        }
    }

    fn cost(self, text: &str) -> usize {
        match self {
            Self::Tokens(_) => approx_token_count(text),
            Self::Characters(_) => text.chars().count(),
        }
    }

    fn cost_from_counts(self, chars: usize, bytes: usize) -> usize {
        match self {
            Self::Tokens(_) => approx_token_count_from_bytes(bytes),
            Self::Characters(_) => chars,
        }
    }
}

fn approx_token_count_from_bytes(bytes: usize) -> usize {
    bytes.saturating_add(APPROX_BYTES_PER_TOKEN.saturating_sub(1)) / APPROX_BYTES_PER_TOKEN
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRenderReport {
    pub total_count: usize,
    pub included_count: usize,
    pub omitted_count: usize,
    pub truncated_description_chars: usize,
    pub truncated_description_count: usize,
}

#[derive(Clone, Copy)]
pub enum SkillRenderSideEffects<'a> {
    None,
    ThreadStart {
        session_telemetry: &'a SessionTelemetry,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableSkills {
    pub skill_root_lines: Vec<String>,
    pub skill_lines: Vec<String>,
    pub report: SkillRenderReport,
    pub warning_message: Option<String>,
}

pub fn default_skill_metadata_budget(context_window: Option<i64>) -> SkillMetadataBudget {
    context_window
        .and_then(|window| usize::try_from(window).ok())
        .filter(|window| *window > 0)
        .map(|window| {
            SkillMetadataBudget::Tokens(
                window
                    .saturating_mul(SKILL_METADATA_CONTEXT_WINDOW_PERCENT)
                    .saturating_div(100)
                    .max(1),
            )
        })
        .unwrap_or(SkillMetadataBudget::Characters(
            DEFAULT_SKILL_METADATA_CHAR_BUDGET,
        ))
}

pub fn build_available_skills(
    outcome: &SkillLoadOutcome,
    budget: SkillMetadataBudget,
    side_effects: SkillRenderSideEffects<'_>,
) -> Option<AvailableSkills> {
    let skills = outcome.allowed_skills_for_implicit_invocation();
    if skills.is_empty() {
        record_skill_render_side_effects(
            side_effects,
            /*total_count*/ 0,
            /*included_count*/ 0,
            /*omitted_count*/ 0,
            /*truncated_description_chars*/ 0,
        );
        return None;
    }

    let absolute_lines = ordered_absolute_skill_lines(&skills);
    let absolute = build_available_skills_from_lines(
        absolute_lines,
        skills.len(),
        budget,
        SkillPathAliases::default(),
    )
    .expect("non-empty skill list should render");

    let selected =
        if absolute.report.omitted_count == 0 && absolute.report.truncated_description_chars == 0 {
            absolute
        } else if let Some(aliased) = build_aliased_available_skills(outcome, &skills, budget) {
            if aliased_render_is_better(&aliased, &absolute, budget) {
                aliased
            } else {
                absolute
            }
        } else {
            absolute
        };

    record_available_skills_side_effects(&selected, budget, side_effects);
    Some(selected)
}

fn build_available_skills_from_lines(
    skill_lines: Vec<SkillLine<'_>>,
    total_count: usize,
    budget: SkillMetadataBudget,
    path_aliases: SkillPathAliases,
) -> Option<AvailableSkills> {
    if total_count == 0 {
        return None;
    }

    let (skill_lines, report) = render_skill_lines_from_lines(skill_lines, total_count, budget);
    let warning_message = if report.omitted_count > 0 {
        let skill_word = if report.omitted_count == 1 {
            "skill"
        } else {
            "skills"
        };
        let verb = if report.omitted_count == 1 {
            "was"
        } else {
            "were"
        };
        Some(format!(
            "{} {} additional {} {} not included in the model-visible skills list.",
            budget_warning_prefix(budget, SKILL_DESCRIPTIONS_REMOVED_WARNING_PREFIX),
            report.omitted_count,
            skill_word,
            verb
        ))
    } else if report.average_truncated_description_chars()
        > SKILL_DESCRIPTION_TRUNCATION_WARNING_THRESHOLD_CHARS
    {
        Some(format!(
            "{} {} characters per skill.",
            budget_warning_prefix(budget, SKILL_DESCRIPTION_TRUNCATED_WARNING_PREFIX),
            report.average_truncated_description_chars()
        ))
    } else {
        None
    };
    let available = AvailableSkills {
        skill_root_lines: path_aliases.skill_root_lines,
        skill_lines,
        report,
        warning_message,
    };
    Some(available)
}

fn record_available_skills_side_effects(
    available: &AvailableSkills,
    budget: SkillMetadataBudget,
    side_effects: SkillRenderSideEffects<'_>,
) {
    record_skill_render_side_effects(
        side_effects,
        available.report.total_count,
        available.report.included_count,
        available.report.omitted_count,
        available.report.truncated_description_chars,
    );
    if available.report.omitted_count > 0 || available.report.truncated_description_chars > 0 {
        tracing::info!(
            budget_limit = budget.limit(),
            total_skills = available.report.total_count,
            included_skills = available.report.included_count,
            omitted_skills = available.report.omitted_count,
            truncated_description_chars_per_skill =
                available.report.average_truncated_description_chars(),
            truncated_skill_descriptions = available.report.truncated_description_count,
            "truncated skill metadata to fit skills context budget"
        );
    }
}

fn budget_warning_prefix(budget: SkillMetadataBudget, prefix: &str) -> String {
    match budget {
        SkillMetadataBudget::Tokens(_) => prefix.replacen(
            "Exceeded skills context budget.",
            "Exceeded skills context budget of 2%.",
            1,
        ),
        SkillMetadataBudget::Characters(_) => prefix.to_string(),
    }
}

fn record_skill_render_side_effects(
    side_effects: SkillRenderSideEffects<'_>,
    total_count: usize,
    included_count: usize,
    omitted_count: usize,
    truncated_description_chars: usize,
) {
    match side_effects {
        SkillRenderSideEffects::None => {}
        SkillRenderSideEffects::ThreadStart { session_telemetry } => {
            session_telemetry.histogram(
                THREAD_SKILLS_ENABLED_TOTAL_METRIC,
                i64::try_from(total_count).unwrap_or(i64::MAX),
                &[],
            );
            session_telemetry.histogram(
                THREAD_SKILLS_KEPT_TOTAL_METRIC,
                i64::try_from(included_count).unwrap_or(i64::MAX),
                &[],
            );
            session_telemetry.histogram(
                THREAD_SKILLS_TRUNCATED_METRIC,
                if omitted_count > 0 { 1 } else { 0 },
                &[],
            );
            session_telemetry.histogram(
                THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC,
                i64::try_from(truncated_description_chars).unwrap_or(i64::MAX),
                &[],
            );
        }
    }
}

fn render_skill_lines_from_lines(
    skill_lines: Vec<SkillLine<'_>>,
    total_count: usize,
    budget: SkillMetadataBudget,
) -> (Vec<String>, SkillRenderReport) {
    let full_cost = skill_lines.iter().fold(0usize, |used, line| {
        used.saturating_add(line.full_cost(budget))
    });
    if full_cost <= budget.limit() {
        let included = skill_lines
            .iter()
            .map(SkillLine::render_full)
            .collect::<Vec<_>>();

        return (
            included,
            skill_render_report(
                total_count,
                /*included_count*/ skill_lines.len(),
                /*omitted_count*/ 0,
                /*truncated_description_chars*/ 0,
                /*truncated_description_count*/ 0,
            ),
        );
    }

    let minimum_cost = skill_lines.iter().fold(0usize, |used, line| {
        used.saturating_add(line.minimum_cost(budget))
    });
    if minimum_cost <= budget.limit() {
        let rendered = render_lines_with_description_budget(
            budget,
            &skill_lines,
            budget.limit().saturating_sub(minimum_cost),
        );
        let (truncated_description_chars, truncated_description_count) =
            sum_description_truncation(&rendered);
        let included = rendered
            .into_iter()
            .map(|rendered| rendered.line)
            .collect::<Vec<_>>();

        return (
            included,
            skill_render_report(
                total_count,
                /*included_count*/ skill_lines.len(),
                /*omitted_count*/ 0,
                truncated_description_chars,
                truncated_description_count,
            ),
        );
    }

    render_minimum_skill_lines_until_budget(budget, skill_lines, total_count)
}

fn render_minimum_skill_lines_until_budget(
    budget: SkillMetadataBudget,
    skill_lines: Vec<SkillLine<'_>>,
    total_count: usize,
) -> (Vec<String>, SkillRenderReport) {
    let mut included = Vec::new();
    let mut used = 0usize;
    let mut omitted_count = 0usize;
    let mut truncated_description_chars = 0usize;
    let mut truncated_description_count = 0usize;
    for line in skill_lines {
        let line_cost = line.minimum_cost(budget);
        let description_char_count = line.description_char_count();
        if used.saturating_add(line_cost) <= budget.limit() {
            used = used.saturating_add(line_cost);
            included.push(line.render_minimum());
        } else {
            omitted_count = omitted_count.saturating_add(1);
        }

        truncated_description_chars =
            truncated_description_chars.saturating_add(description_char_count);
        if description_char_count > 0 {
            truncated_description_count = truncated_description_count.saturating_add(1);
        }
    }

    let report = skill_render_report(
        total_count,
        included.len(),
        omitted_count,
        truncated_description_chars,
        truncated_description_count,
    );

    (included, report)
}

fn skill_render_report(
    total_count: usize,
    included_count: usize,
    omitted_count: usize,
    truncated_description_chars: usize,
    truncated_description_count: usize,
) -> SkillRenderReport {
    SkillRenderReport {
        total_count,
        included_count,
        omitted_count,
        truncated_description_chars,
        truncated_description_count,
    }
}

impl SkillRenderReport {
    fn average_truncated_description_chars(&self) -> usize {
        if self.truncated_description_count == 0 {
            return 0;
        }

        self.truncated_description_chars
            .saturating_add(self.truncated_description_count.saturating_sub(1))
            / self.truncated_description_count
    }
}

struct SkillLine<'a> {
    name: &'a str,
    description: &'a str,
    path: String,
}

struct RenderedSkillLine {
    line: String,
    truncated_chars: usize,
}

struct DescriptionBudgetLine<'a> {
    line: &'a SkillLine<'a>,
    description_char_count: usize,
    extra_costs: Vec<usize>,
}

fn sum_description_truncation(rendered: &[RenderedSkillLine]) -> (usize, usize) {
    rendered
        .iter()
        .fold((0usize, 0usize), |(chars, count), line| {
            if line.truncated_chars == 0 {
                (chars, count)
            } else {
                (
                    chars.saturating_add(line.truncated_chars),
                    count.saturating_add(1),
                )
            }
        })
}

impl<'a> SkillLine<'a> {
    fn new(skill: &'a SkillMetadata) -> Self {
        Self::with_path(
            skill,
            skill.path_to_skills_md.to_string_lossy().replace('\\', "/"),
        )
    }

    fn with_path(skill: &'a SkillMetadata, path: String) -> Self {
        Self {
            name: skill.name.as_str(),
            description: skill.description.as_str(),
            path,
        }
    }

    fn full_cost(&self, budget: SkillMetadataBudget) -> usize {
        line_cost(budget, &self.render_full())
    }

    fn minimum_cost(&self, budget: SkillMetadataBudget) -> usize {
        line_cost(budget, &self.render_minimum())
    }

    fn description_char_count(&self) -> usize {
        self.description.chars().count()
    }

    fn render_full(&self) -> String {
        self.render_with_description(self.description)
    }

    fn render_minimum(&self) -> String {
        self.render_with_description("")
    }

    fn rendered_description_prefix_len(&self, description_chars: usize) -> usize {
        self.description
            .char_indices()
            .nth(description_chars)
            .map_or(self.description.len(), |(idx, _)| idx)
    }

    fn render_with_description_chars(&self, description_chars: usize) -> String {
        if description_chars == 0 {
            format!("- {}: (file: {})", self.name, self.path)
        } else {
            let end = self.rendered_description_prefix_len(description_chars);
            let description = &self.description[..end];
            format!("- {}: {} (file: {})", self.name, description, self.path)
        }
    }

    fn render_with_description(&self, description: &str) -> String {
        if description.is_empty() {
            format!("- {}: (file: {})", self.name, self.path)
        } else {
            format!("- {}: {} (file: {})", self.name, description, self.path)
        }
    }
}

impl<'a> DescriptionBudgetLine<'a> {
    fn new(line: &'a SkillLine<'a>, budget: SkillMetadataBudget) -> Self {
        let minimum_line = line.render_minimum();
        let minimum_chars = minimum_line.chars().count().saturating_add(1);
        let minimum_bytes = minimum_line.len().saturating_add(1);
        let minimum_cost = budget.cost_from_counts(minimum_chars, minimum_bytes);

        let description_char_count = line.description_char_count();
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

fn line_cost(budget: SkillMetadataBudget, line: &str) -> usize {
    budget.cost(&format!("{line}\n"))
}

fn lines_cost(budget: SkillMetadataBudget, lines: &[String]) -> usize {
    lines.iter().fold(0usize, |used, line| {
        used.saturating_add(line_cost(budget, line))
    })
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

    // Distribute description space one character at a time across skills.
    // Short descriptions naturally drop out, so their unused share can go to
    // longer descriptions instead of being stranded in a fixed per-skill quota.
    loop {
        let mut changed = false;
        for (index, line) in budget_lines.iter().enumerate() {
            if char_allocations[index] >= line.description_char_count {
                continue;
            }

            let current_cost = current_extra_costs[index];
            let next_chars = char_allocations[index].saturating_add(1);
            let next_cost = line.extra_costs[next_chars];
            let delta = next_cost.saturating_sub(current_cost);
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
        .map(|(line, description_chars)| {
            let truncated_chars = line
                .description_char_count
                .saturating_sub(description_chars);
            RenderedSkillLine {
                line: line.line.render_with_description_chars(description_chars),
                truncated_chars,
            }
        })
        .collect()
}

fn build_aliased_available_skills(
    outcome: &SkillLoadOutcome,
    skills: &[SkillMetadata],
    budget: SkillMetadataBudget,
) -> Option<AvailableSkills> {
    let plan = build_alias_plan(outcome, skills, budget)?;
    if plan.table_cost >= budget.limit() {
        return None;
    }

    let adjusted_limit = budget.limit().saturating_sub(plan.table_cost);
    let adjusted_budget = match budget {
        SkillMetadataBudget::Tokens(_) => SkillMetadataBudget::Tokens(adjusted_limit),
        SkillMetadataBudget::Characters(_) => SkillMetadataBudget::Characters(adjusted_limit),
    };
    let ordered_skills = ordered_skills_for_budget(skills);
    let skill_lines = ordered_skills
        .into_iter()
        .map(|skill| SkillLine::with_path(skill, render_skill_path_with_aliases(skill, &plan)))
        .collect::<Vec<_>>();
    build_available_skills_from_lines(skill_lines, skills.len(), adjusted_budget, plan.aliases)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SkillPathAliases {
    skill_root_lines: Vec<String>,
}

struct AliasPlan {
    aliases: SkillPathAliases,
    root_aliases: HashMap<AbsolutePathBuf, String>,
    alias_root_by_path: HashMap<AbsolutePathBuf, AbsolutePathBuf>,
    table_cost: usize,
}

fn build_alias_plan(
    outcome: &SkillLoadOutcome,
    skills: &[SkillMetadata],
    budget: SkillMetadataBudget,
) -> Option<AliasPlan> {
    let skill_paths = skills
        .iter()
        .map(|skill| skill.path_to_skills_md.clone())
        .collect::<HashSet<_>>();
    let skill_root_by_path = outcome
        .skill_root_by_path
        .iter()
        .filter(|(path, _)| skill_paths.contains(*path))
        .map(|(path, root)| (path.clone(), root.clone()))
        .collect::<HashMap<_, _>>();
    let used_roots = outcome
        .skill_roots
        .iter()
        .filter(|root| {
            skill_root_by_path
                .values()
                .any(|skill_root| skill_root == *root)
        })
        .cloned()
        .collect::<Vec<_>>();
    if used_roots.is_empty() {
        return None;
    }

    let alias_root_by_skill_root = used_roots
        .iter()
        .map(|root| (root.clone(), alias_root_for_skill_root(root)))
        .collect::<HashMap<_, _>>();
    let alias_roots = ordered_alias_roots(&used_roots, &alias_root_by_skill_root);
    let root_aliases = alias_roots
        .iter()
        .enumerate()
        .map(|(index, alias_root)| (alias_root.clone(), format!("r{index}")))
        .collect::<HashMap<_, _>>();
    let alias_root_by_path = skill_root_by_path
        .iter()
        .filter_map(|(path, skill_root)| {
            alias_root_by_skill_root
                .get(skill_root)
                .map(|alias_root| (path.clone(), alias_root.clone()))
        })
        .collect::<HashMap<_, _>>();
    let skill_root_lines = build_skill_root_lines(&alias_roots);
    let table_cost = aliased_metadata_overhead_cost(budget, &skill_root_lines);

    Some(AliasPlan {
        aliases: SkillPathAliases { skill_root_lines },
        root_aliases,
        alias_root_by_path,
        table_cost,
    })
}

fn ordered_alias_roots(
    used_roots: &[AbsolutePathBuf],
    alias_root_by_skill_root: &HashMap<AbsolutePathBuf, AbsolutePathBuf>,
) -> Vec<AbsolutePathBuf> {
    let mut seen = HashSet::new();
    let mut alias_roots = Vec::new();
    for root in used_roots {
        let alias_root = alias_root_by_skill_root
            .get(root)
            .expect("alias root should exist for every used root")
            .clone();
        if seen.insert(alias_root.clone()) {
            alias_roots.push(alias_root);
        }
    }
    alias_roots
}

fn alias_root_for_skill_root(root: &AbsolutePathBuf) -> AbsolutePathBuf {
    plugin_marketplace_base(root.as_path()).unwrap_or_else(|| root.clone())
}

fn aliased_metadata_overhead_cost(
    budget: SkillMetadataBudget,
    skill_root_lines: &[String],
) -> usize {
    alias_instruction_overhead_cost(budget).saturating_add(lines_cost(budget, skill_root_lines))
}

fn alias_instruction_overhead_cost(budget: SkillMetadataBudget) -> usize {
    let absolute_lines = [
        SKILLS_INTRO_WITH_ABSOLUTE_PATHS.to_string(),
        SKILLS_HOW_TO_USE_WITH_ABSOLUTE_PATHS.to_string(),
    ];
    let aliased_lines = vec![
        SKILLS_INTRO_WITH_ALIASES.to_string(),
        "### Skill roots".to_string(),
        SKILLS_HOW_TO_USE_WITH_ALIASES.to_string(),
    ];

    lines_cost(budget, &aliased_lines).saturating_sub(lines_cost(budget, &absolute_lines))
}

fn build_skill_root_lines(roots: &[AbsolutePathBuf]) -> Vec<String> {
    roots
        .iter()
        .enumerate()
        .map(|(index, root)| {
            let root_str = root.to_string_lossy().replace('\\', "/");
            format!("- `r{index}` = `{root_str}`")
        })
        .collect()
}

fn plugin_marketplace_base(path: &Path) -> Option<AbsolutePathBuf> {
    let mut candidate = path;
    while let Some(parent) = candidate.parent() {
        if parent.file_name()?.to_str()? == "cache"
            && parent.parent()?.file_name()?.to_str()? == "plugins"
        {
            return AbsolutePathBuf::from_absolute_path(candidate).ok();
        }
        candidate = parent;
    }
    None
}

fn render_skill_path_with_aliases(skill: &SkillMetadata, plan: &AliasPlan) -> String {
    outcome_relative_skill_path(skill, plan)
        .unwrap_or_else(|| skill.path_to_skills_md.to_string_lossy().replace('\\', "/"))
}

fn outcome_relative_skill_path(skill: &SkillMetadata, plan: &AliasPlan) -> Option<String> {
    let alias_root = plan.alias_root_by_path.get(&skill.path_to_skills_md)?;
    let alias = plan.root_aliases.get(alias_root)?;
    let relative_path = skill
        .path_to_skills_md
        .as_path()
        .strip_prefix(alias_root.as_path())
        .ok()?;
    let relative_path = relative_path.to_string_lossy().replace('\\', "/");
    Some(format!("{alias}/{relative_path}"))
}

fn aliased_render_is_better(
    aliased: &AvailableSkills,
    absolute: &AvailableSkills,
    budget: SkillMetadataBudget,
) -> bool {
    if aliased.report.included_count != absolute.report.included_count {
        return aliased.report.included_count > absolute.report.included_count;
    }
    if aliased.report.truncated_description_chars != absolute.report.truncated_description_chars {
        return aliased.report.truncated_description_chars
            < absolute.report.truncated_description_chars;
    }
    available_skills_cost(budget, aliased) < available_skills_cost(budget, absolute)
}

fn available_skills_cost(budget: SkillMetadataBudget, available: &AvailableSkills) -> usize {
    let metadata_cost = if available.skill_root_lines.is_empty() {
        0
    } else {
        aliased_metadata_overhead_cost(budget, &available.skill_root_lines)
    };
    metadata_cost.saturating_add(lines_cost(budget, &available.skill_lines))
}

fn ordered_absolute_skill_lines(skills: &[SkillMetadata]) -> Vec<SkillLine<'_>> {
    ordered_skills_for_budget(skills)
        .into_iter()
        .map(SkillLine::new)
        .collect()
}

fn ordered_skills_for_budget(skills: &[SkillMetadata]) -> Vec<&SkillMetadata> {
    let mut ordered = skills.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| {
        prompt_scope_rank(a.scope)
            .cmp(&prompt_scope_rank(b.scope))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path_to_skills_md.cmp(&b.path_to_skills_md))
    });
    ordered
}

fn prompt_scope_rank(scope: SkillScope) -> u8 {
    match scope {
        SkillScope::System => 0,
        SkillScope::Admin => 1,
        SkillScope::Repo => 2,
        SkillScope::User => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    fn make_skill(name: &str, scope: SkillScope) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: "desc".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: test_path_buf(&format!("/tmp/{name}/SKILL.md")).abs(),
            scope,
        }
    }

    fn make_skill_with_description(
        name: &str,
        scope: SkillScope,
        description: &str,
    ) -> SkillMetadata {
        let mut skill = make_skill(name, scope);
        skill.description = description.to_string();
        skill
    }

    fn expected_skill_line(skill: &SkillMetadata, description: &str) -> String {
        SkillLine::new(skill).render_with_description(description)
    }

    fn normalized_path(path: &AbsolutePathBuf) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    fn outcome_with_roots(
        skills: Vec<SkillMetadata>,
        roots: Vec<AbsolutePathBuf>,
    ) -> SkillLoadOutcome {
        let skill_root_by_path = skills
            .iter()
            .filter_map(|skill| {
                roots
                    .iter()
                    .find(|root| {
                        skill
                            .path_to_skills_md
                            .as_path()
                            .starts_with(root.as_path())
                    })
                    .map(|root| (skill.path_to_skills_md.clone(), root.clone()))
            })
            .collect::<HashMap<_, _>>();
        SkillLoadOutcome {
            skills,
            skill_roots: roots,
            skill_root_by_path: Arc::new(skill_root_by_path),
            ..Default::default()
        }
    }

    fn build_available_skills_from_metadata(
        skills: &[SkillMetadata],
        budget: SkillMetadataBudget,
    ) -> Option<AvailableSkills> {
        build_available_skills_from_lines(
            ordered_absolute_skill_lines(skills),
            skills.len(),
            budget,
            SkillPathAliases::default(),
        )
    }

    #[test]
    fn default_budget_uses_two_percent_of_full_context_window() {
        assert_eq!(
            default_skill_metadata_budget(Some(200_000)),
            SkillMetadataBudget::Tokens(4_000)
        );
        assert_eq!(
            default_skill_metadata_budget(Some(99)),
            SkillMetadataBudget::Tokens(1)
        );
    }

    #[test]
    fn default_budget_falls_back_to_characters_without_context_window() {
        assert_eq!(
            default_skill_metadata_budget(/*context_window*/ None),
            SkillMetadataBudget::Characters(DEFAULT_SKILL_METADATA_CHAR_BUDGET)
        );
        assert_eq!(
            default_skill_metadata_budget(Some(-1)),
            SkillMetadataBudget::Characters(DEFAULT_SKILL_METADATA_CHAR_BUDGET)
        );
    }

    #[test]
    fn budgeted_rendering_truncates_descriptions_equally_before_omitting_skills() {
        let alpha = make_skill_with_description("alpha-skill", SkillScope::Repo, "abcdef");
        let beta = make_skill_with_description("beta-skill", SkillScope::Repo, "uvwxyz");
        let minimum_cost = SkillLine::new(&alpha)
            .minimum_cost(SkillMetadataBudget::Characters(usize::MAX))
            + SkillLine::new(&beta).minimum_cost(SkillMetadataBudget::Characters(usize::MAX));
        let budget = SkillMetadataBudget::Characters(minimum_cost + 6);

        let rendered = build_available_skills_from_metadata(&[beta.clone(), alpha.clone()], budget)
            .expect("skills should render");

        assert_eq!(rendered.report.included_count, 2);
        assert_eq!(rendered.report.omitted_count, 0);
        assert_eq!(rendered.report.truncated_description_chars, 8);
        assert_eq!(rendered.warning_message, None);
        assert_eq!(
            rendered.skill_lines,
            vec![
                expected_skill_line(&alpha, "ab"),
                expected_skill_line(&beta, "uv"),
            ]
        );
    }

    #[test]
    fn budgeted_rendering_does_not_warn_when_average_description_truncation_is_within_threshold() {
        let alpha = make_skill_with_description("alpha-skill", SkillScope::Repo, "abcdefghij");
        let beta = make_skill_with_description("beta-skill", SkillScope::Repo, "uvwxyzabcd");
        let minimum_cost = SkillLine::new(&alpha)
            .minimum_cost(SkillMetadataBudget::Characters(usize::MAX))
            + SkillLine::new(&beta).minimum_cost(SkillMetadataBudget::Characters(usize::MAX));
        let budget = SkillMetadataBudget::Characters(minimum_cost + 6);

        let rendered = build_available_skills_from_metadata(&[alpha, beta], budget)
            .expect("skills should render");

        assert_eq!(rendered.report.included_count, 2);
        assert_eq!(rendered.report.omitted_count, 0);
        assert_eq!(rendered.report.truncated_description_chars, 16);
        assert_eq!(rendered.report.truncated_description_count, 2);
        assert_eq!(rendered.warning_message, None);
    }

    #[test]
    fn budgeted_rendering_warns_when_average_description_truncation_exceeds_threshold() {
        let alpha =
            make_skill_with_description("alpha-skill", SkillScope::Repo, "abcdefghijklmnop");
        let beta = make_skill_with_description("beta-skill", SkillScope::Repo, "uvwxyzabcdefghij");
        let minimum_cost = SkillLine::new(&alpha)
            .minimum_cost(SkillMetadataBudget::Characters(usize::MAX))
            + SkillLine::new(&beta).minimum_cost(SkillMetadataBudget::Characters(usize::MAX));
        let budget = SkillMetadataBudget::Characters(minimum_cost + 6);

        let rendered = build_available_skills_from_metadata(&[alpha, beta], budget)
            .expect("skills should render");

        assert_eq!(rendered.report.included_count, 2);
        assert_eq!(rendered.report.omitted_count, 0);
        assert_eq!(rendered.report.truncated_description_chars, 28);
        assert_eq!(rendered.report.truncated_description_count, 2);
        assert_eq!(
            rendered.warning_message,
            Some(
                "Warning: Exceeded skills context budget. Loaded skill descriptions were truncated by an average of 14 characters per skill."
                    .to_string()
            )
        );
    }

    #[test]
    fn budgeted_rendering_redistributes_unused_description_budget() {
        let short = make_skill_with_description("short-skill", SkillScope::Repo, "x");
        let long = make_skill_with_description("long-skill", SkillScope::Repo, "abcdefghi");
        let minimum_cost = SkillLine::new(&short)
            .minimum_cost(SkillMetadataBudget::Characters(usize::MAX))
            + SkillLine::new(&long).minimum_cost(SkillMetadataBudget::Characters(usize::MAX));
        let budget = SkillMetadataBudget::Characters(minimum_cost + 11);

        let rendered = build_available_skills_from_metadata(&[short.clone(), long.clone()], budget)
            .expect("skills should render");

        assert_eq!(rendered.report.included_count, 2);
        assert_eq!(rendered.report.omitted_count, 0);
        assert_eq!(rendered.warning_message, None);
        assert_eq!(
            rendered.skill_lines,
            vec![
                expected_skill_line(&long, "abcdefgh"),
                expected_skill_line(&short, "x"),
            ]
        );
    }

    #[test]
    fn budgeted_rendering_preserves_prompt_priority_when_minimum_lines_exceed_budget() {
        let system = make_skill("system-skill", SkillScope::System);
        let user = make_skill("user-skill", SkillScope::User);
        let repo = make_skill("repo-skill", SkillScope::Repo);
        let admin = make_skill("admin-skill", SkillScope::Admin);
        let system_cost = SkillMetadataBudget::Characters(usize::MAX)
            .cost(&format!("{}\n", SkillLine::new(&system).render_minimum()));
        let admin_cost = SkillMetadataBudget::Characters(usize::MAX)
            .cost(&format!("{}\n", SkillLine::new(&admin).render_minimum()));
        let budget = SkillMetadataBudget::Characters(system_cost + admin_cost);

        let rendered = build_available_skills_from_metadata(&[system, user, repo, admin], budget)
            .expect("skills should render");

        assert_eq!(rendered.report.included_count, 2);
        assert_eq!(rendered.report.omitted_count, 2);
        assert_eq!(
            rendered.warning_message,
            Some(
                "Warning: Exceeded skills context budget. All skill descriptions were removed and 2 additional skills were not included in the model-visible skills list."
                    .to_string()
            )
        );
        let rendered_text = rendered.skill_lines.join("\n");
        assert!(rendered_text.contains("- system-skill:"));
        assert!(rendered_text.contains("- admin-skill:"));
        assert!(!rendered_text.contains("desc"));
        assert!(!rendered_text.contains("- repo-skill:"));
        assert!(!rendered_text.contains("- user-skill:"));
    }

    #[test]
    fn budgeted_rendering_keeps_scanning_after_oversized_entry() {
        let mut oversized = make_skill("oversized-system-skill", SkillScope::System);
        oversized.description = "desc ".repeat(100);
        let repo = make_skill("repo-skill", SkillScope::Repo);
        let repo_cost = SkillMetadataBudget::Characters(usize::MAX)
            .cost(&format!("{}\n", SkillLine::new(&repo).render_full()));
        let budget = SkillMetadataBudget::Characters(repo_cost);

        let rendered = build_available_skills_from_metadata(&[oversized, repo], budget)
            .expect("skills render");

        assert_eq!(rendered.report.included_count, 1);
        assert_eq!(rendered.report.omitted_count, 1);
        assert_eq!(
            rendered.warning_message,
            Some(
                "Warning: Exceeded skills context budget. All skill descriptions were removed and 1 additional skill was not included in the model-visible skills list."
                    .to_string()
            )
        );
        let rendered_text = rendered.skill_lines.join("\n");
        assert!(!rendered_text.contains("- oversized-system-skill:"));
        assert!(rendered_text.contains("- repo-skill:"));
    }

    #[test]
    fn outcome_rendering_omits_aliases_when_absolute_plan_has_no_budget_pressure() {
        let root = test_path_buf("/tmp/skills").abs();
        let alpha_path = root.join("alpha/SKILL.md");
        let beta_path = root.join("beta/SKILL.md");
        let outcome = outcome_with_roots(
            vec![
                skill_with_path("alpha-skill", &alpha_path),
                skill_with_path("beta-skill", &beta_path),
            ],
            vec![root],
        );

        let rendered = build_available_skills(
            &outcome,
            SkillMetadataBudget::Characters(usize::MAX),
            SkillRenderSideEffects::None,
        )
        .expect("skills should render");

        assert!(rendered.skill_root_lines.is_empty());
        assert_eq!(rendered.report.included_count, 2);
    }

    #[test]
    fn outcome_rendering_uses_aliases_when_they_allow_more_skills_to_fit() {
        let root = test_path_buf(
            "/Users/xl/.codex/plugins/cache/openai-curated/example/hash1234567890/skills-with-a-very-long-shared-prefix",
        )
        .abs();
        let skills = (0..12)
            .map(|index| {
                let name = format!("shared-root-skill-{index}");
                skill_with_path(&name, &root.join(format!("skill-{index}/SKILL.md")))
            })
            .collect::<Vec<_>>();
        let outcome = outcome_with_roots(skills.clone(), vec![root.clone()]);
        let absolute_minimum = skills.iter().fold(0usize, |cost, skill| {
            cost.saturating_add(
                SkillLine::new(skill).minimum_cost(SkillMetadataBudget::Characters(usize::MAX)),
            )
        });
        let plan = build_alias_plan(
            &outcome,
            &skills,
            SkillMetadataBudget::Characters(usize::MAX),
        )
        .expect("alias plan should build");
        let alias_minimum = skills.iter().fold(plan.table_cost, |cost, skill| {
            cost.saturating_add(
                SkillLine::with_path(skill, render_skill_path_with_aliases(skill, &plan))
                    .minimum_cost(SkillMetadataBudget::Characters(usize::MAX)),
            )
        });
        assert!(
            alias_minimum < absolute_minimum,
            "test fixture should make aliases cheaper"
        );

        let rendered = build_available_skills(
            &outcome,
            SkillMetadataBudget::Characters(alias_minimum),
            SkillRenderSideEffects::None,
        )
        .expect("skills should render");

        assert_eq!(rendered.report.included_count, skills.len());
        assert_eq!(rendered.report.omitted_count, 0);
        assert_eq!(
            rendered.skill_root_lines,
            vec![format!(
                "- `r0` = `{}`",
                normalized_path(
                    &test_path_buf("/Users/xl/.codex/plugins/cache/openai-curated").abs()
                )
            )]
        );
        let rendered_text = rendered.skill_lines.join("\n");
        assert!(rendered_text.contains(
            "r0/example/hash1234567890/skills-with-a-very-long-shared-prefix/skill-0/SKILL.md"
        ));
        assert!(rendered_text.contains(
            "r0/example/hash1234567890/skills-with-a-very-long-shared-prefix/skill-11/SKILL.md"
        ));
    }

    #[test]
    fn outcome_rendering_extracts_plugin_marketplace_root_for_multiple_plugins() {
        let github_root =
            test_path_buf("/Users/xl/.codex/plugins/cache/openai-curated/github/hash123/skills")
                .abs();
        let slack_root =
            test_path_buf("/Users/xl/.codex/plugins/cache/openai-curated/slack/hash456/skills")
                .abs();
        let marketplace_root = test_path_buf("/Users/xl/.codex/plugins/cache/openai-curated").abs();
        let github = skill_with_path("github:gh-fix-ci", &github_root.join("gh-fix-ci/SKILL.md"));
        let slack = skill_with_path(
            "slack:daily-digest",
            &slack_root.join("daily-digest/SKILL.md"),
        );
        let outcome = outcome_with_roots(
            vec![github.clone(), slack.clone()],
            vec![github_root.clone(), slack_root.clone()],
        );
        let plan = build_alias_plan(
            &outcome,
            &[github, slack],
            SkillMetadataBudget::Characters(usize::MAX),
        )
        .expect("alias plan should build");

        assert_eq!(
            plan.aliases.skill_root_lines,
            vec![format!("- `r0` = `{}`", normalized_path(&marketplace_root))]
        );
        assert_eq!(
            render_skill_path_with_aliases(
                &skill_with_path("github:gh-fix-ci", &github_root.join("gh-fix-ci/SKILL.md")),
                &plan
            ),
            "r0/github/hash123/skills/gh-fix-ci/SKILL.md"
        );
        assert_eq!(
            render_skill_path_with_aliases(
                &skill_with_path(
                    "slack:daily-digest",
                    &slack_root.join("daily-digest/SKILL.md")
                ),
                &plan
            ),
            "r0/slack/hash456/skills/daily-digest/SKILL.md"
        );
    }

    #[test]
    fn outcome_rendering_uses_one_marketplace_root_for_multiple_plugin_versions() {
        let skills_root =
            test_path_buf("/Users/xl/.codex/plugins/cache/openai-curated/github/hash123/skills")
                .abs();
        let extra_root = test_path_buf(
            "/Users/xl/.codex/plugins/cache/openai-curated/github/hash456/extra-skills",
        )
        .abs();
        let marketplace_root = test_path_buf("/Users/xl/.codex/plugins/cache/openai-curated").abs();
        let fix_ci = skill_with_path("github:gh-fix-ci", &skills_root.join("gh-fix-ci/SKILL.md"));
        let yeet = skill_with_path("github:yeet", &extra_root.join("yeet/SKILL.md"));
        let outcome = outcome_with_roots(
            vec![fix_ci.clone(), yeet.clone()],
            vec![skills_root.clone(), extra_root.clone()],
        );
        let plan = build_alias_plan(
            &outcome,
            &[fix_ci, yeet],
            SkillMetadataBudget::Characters(usize::MAX),
        )
        .expect("alias plan should build");

        assert_eq!(
            plan.aliases.skill_root_lines,
            vec![format!("- `r0` = `{}`", normalized_path(&marketplace_root))]
        );
        assert_eq!(
            render_skill_path_with_aliases(
                &skill_with_path("github:gh-fix-ci", &skills_root.join("gh-fix-ci/SKILL.md")),
                &plan
            ),
            "r0/github/hash123/skills/gh-fix-ci/SKILL.md"
        );
        assert_eq!(
            render_skill_path_with_aliases(
                &skill_with_path("github:yeet", &extra_root.join("yeet/SKILL.md")),
                &plan
            ),
            "r0/github/hash456/extra-skills/yeet/SKILL.md"
        );
    }

    fn skill_with_path(name: &str, path: &AbsolutePathBuf) -> SkillMetadata {
        let mut skill = make_skill(name, SkillScope::User);
        skill.path_to_skills_md = path.clone();
        skill
    }
}
