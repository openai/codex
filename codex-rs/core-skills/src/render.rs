use crate::model::SkillMetadata;
use codex_otel::SessionTelemetry;
use codex_otel::THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC;
use codex_otel::THREAD_SKILLS_ENABLED_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_KEPT_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_TRUNCATED_METRIC;
use codex_protocol::protocol::SkillScope;
use codex_utils_output_truncation::approx_token_count;

const DEFAULT_SKILL_METADATA_CHAR_BUDGET: usize = 8_000;
const SKILL_METADATA_CONTEXT_WINDOW_PERCENT: usize = 2;
const SKILL_DESCRIPTION_TRUNCATION_WARNING_THRESHOLD_CHARS: usize = 10;
pub const SKILL_DESCRIPTION_TRUNCATED_WARNING_PREFIX: &str = "Warning: Exceeded skills context budget. Loaded skill descriptions were truncated by an average of";
pub const SKILL_DESCRIPTIONS_REMOVED_WARNING_PREFIX: &str =
    "Warning: Exceeded skills context budget. All skill descriptions were removed and";

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
    skills: &[SkillMetadata],
    budget: SkillMetadataBudget,
    side_effects: SkillRenderSideEffects<'_>,
) -> Option<AvailableSkills> {
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

    let (skill_lines, report) = render_skill_lines(skills, budget);
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
    } else if report.truncated_description_chars
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
    record_skill_render_side_effects(
        side_effects,
        report.total_count,
        report.included_count,
        report.omitted_count,
        report.truncated_description_chars,
    );
    if report.omitted_count > 0 || report.truncated_description_chars > 0 {
        tracing::info!(
            budget_limit = budget.limit(),
            total_skills = report.total_count,
            included_skills = report.included_count,
            omitted_skills = report.omitted_count,
            truncated_description_chars_per_skill = report.average_truncated_description_chars(),
            truncated_skill_descriptions = report.truncated_description_count,
            "truncated skill metadata to fit skills context budget"
        );
    }
    Some(AvailableSkills {
        skill_lines,
        report,
        warning_message,
    })
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

fn render_skill_lines(
    skills: &[SkillMetadata],
    budget: SkillMetadataBudget,
) -> (Vec<String>, SkillRenderReport) {
    let ordered_skills = ordered_skills_for_budget(skills);
    let skill_lines = ordered_skills
        .into_iter()
        .map(SkillLine::new)
        .collect::<Vec<_>>();

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
                /*total_count*/ skills.len(),
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
                /*total_count*/ skills.len(),
                /*included_count*/ skill_lines.len(),
                /*omitted_count*/ 0,
                truncated_description_chars,
                truncated_description_count,
            ),
        );
    }

    render_minimum_skill_lines_until_budget(budget, skill_lines, skills.len())
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
        Self {
            name: skill.name.as_str(),
            description: skill.description.as_str(),
            path: skill.path_to_skills_md.to_string_lossy().replace('\\', "/"),
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

fn line_cost(budget: SkillMetadataBudget, line: &str) -> usize {
    budget.cost(&format!("{line}\n"))
}

fn render_lines_with_description_budget(
    budget: SkillMetadataBudget,
    skill_lines: &[SkillLine<'_>],
    limit: usize,
) -> Vec<RenderedSkillLine> {
    let mut char_allocations = vec![0usize; skill_lines.len()];
    let mut extra_costs = vec![0usize; skill_lines.len()];
    let mut remaining = limit;

    // Distribute description space one character at a time across skills.
    // Short descriptions naturally drop out, so their unused share can go to
    // longer descriptions instead of being stranded in a fixed per-skill quota.
    loop {
        let mut changed = false;
        for (index, line) in skill_lines.iter().enumerate() {
            if char_allocations[index] >= line.description_char_count() {
                continue;
            }

            let current_cost = extra_costs[index];
            let next_chars = char_allocations[index].saturating_add(1);
            let next_cost = line_cost(budget, &line.render_with_description_chars(next_chars))
                .saturating_sub(line.minimum_cost(budget));
            let delta = next_cost.saturating_sub(current_cost);
            if delta <= remaining {
                char_allocations[index] = next_chars;
                extra_costs[index] = next_cost;
                remaining = remaining.saturating_sub(delta);
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    skill_lines
        .iter()
        .zip(char_allocations)
        .map(|(line, description_chars)| {
            let truncated_chars = line
                .description_char_count()
                .saturating_sub(description_chars);
            RenderedSkillLine {
                line: line.render_with_description_chars(description_chars),
                truncated_chars,
            }
        })
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

        let rendered = build_available_skills(&[beta, alpha], budget, SkillRenderSideEffects::None)
            .expect("skills should render");

        assert_eq!(rendered.report.included_count, 2);
        assert_eq!(rendered.report.omitted_count, 0);
        assert_eq!(rendered.report.truncated_description_chars, 8);
        assert_eq!(rendered.warning_message, None);
        assert_eq!(
            rendered.skill_lines,
            vec![
                "- alpha-skill: ab (file: /tmp/alpha-skill/SKILL.md)",
                "- beta-skill: uv (file: /tmp/beta-skill/SKILL.md)",
            ]
        );
    }

    #[test]
    fn budgeted_rendering_warns_when_description_truncation_exceeds_threshold() {
        let alpha = make_skill_with_description("alpha-skill", SkillScope::Repo, "abcdefghij");
        let beta = make_skill_with_description("beta-skill", SkillScope::Repo, "uvwxyzabcd");
        let minimum_cost = SkillLine::new(&alpha)
            .minimum_cost(SkillMetadataBudget::Characters(usize::MAX))
            + SkillLine::new(&beta).minimum_cost(SkillMetadataBudget::Characters(usize::MAX));
        let budget = SkillMetadataBudget::Characters(minimum_cost + 6);

        let rendered = build_available_skills(&[alpha, beta], budget, SkillRenderSideEffects::None)
            .expect("skills should render");

        assert_eq!(rendered.report.included_count, 2);
        assert_eq!(rendered.report.omitted_count, 0);
        assert_eq!(rendered.report.truncated_description_chars, 16);
        assert_eq!(
            rendered.warning_message,
            Some(
                "Warning: Exceeded skills context budget. Loaded skill descriptions were truncated by an average of 8 characters per skill."
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

        let rendered = build_available_skills(&[short, long], budget, SkillRenderSideEffects::None)
            .expect("skills should render");

        assert_eq!(rendered.report.included_count, 2);
        assert_eq!(rendered.report.omitted_count, 0);
        assert_eq!(rendered.warning_message, None);
        assert_eq!(
            rendered.skill_lines,
            vec![
                "- long-skill: abcdefgh (file: /tmp/long-skill/SKILL.md)",
                "- short-skill: x (file: /tmp/short-skill/SKILL.md)",
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

        let rendered = build_available_skills(
            &[system, user, repo, admin],
            budget,
            SkillRenderSideEffects::None,
        )
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

        let rendered =
            build_available_skills(&[oversized, repo], budget, SkillRenderSideEffects::None)
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
}
