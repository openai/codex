use crate::model::SkillMetadata;
use codex_otel::SessionTelemetry;
use codex_otel::THREAD_SKILLS_ENABLED_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_KEPT_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_TRUNCATED_METRIC;
use codex_protocol::protocol::SkillScope;
use codex_utils_output_truncation::approx_token_count;

const DEFAULT_SKILL_METADATA_CHAR_BUDGET: usize = 8_000;
const SKILL_METADATA_CONTEXT_WINDOW_PERCENT: usize = 2;

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
    pub emit_warning: bool,
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
        let _ = record_skill_render_side_effects(
            side_effects,
            /*total_count*/ 0,
            /*included_count*/ 0,
            /*truncated*/ false,
        );
        return None;
    }

    let (skill_lines, report) = render_skill_lines(skills, budget);
    let emit_warning = record_skill_render_side_effects(
        side_effects,
        report.total_count,
        report.included_count,
        report.omitted_count > 0,
    );
    Some(AvailableSkills {
        skill_lines,
        report,
        emit_warning,
    })
}

fn record_skill_render_side_effects(
    side_effects: SkillRenderSideEffects<'_>,
    total_count: usize,
    included_count: usize,
    truncated: bool,
) -> bool {
    match side_effects {
        SkillRenderSideEffects::None => false,
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
                if truncated { 1 } else { 0 },
                &[],
            );
            truncated
        }
    }
}

fn render_skill_lines(
    skills: &[SkillMetadata],
    budget: SkillMetadataBudget,
) -> (Vec<String>, SkillRenderReport) {
    let ordered_skills = ordered_skills_for_budget(skills);

    let mut included = Vec::new();
    let mut used = 0usize;
    let mut omitted_count = 0usize;

    for skill in ordered_skills {
        let line = render_skill_line(skill);
        let line_cost = budget.cost(&format!("{line}\n"));
        if used.saturating_add(line_cost) <= budget.limit() {
            used = used.saturating_add(line_cost);
            included.push(line);
            continue;
        }

        omitted_count = omitted_count.saturating_add(1);
    }

    let report = SkillRenderReport {
        total_count: skills.len(),
        included_count: included.len(),
        omitted_count,
    };

    (included, report)
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

fn render_skill_line(skill: &SkillMetadata) -> String {
    let path_str = skill.path_to_skills_md.to_string_lossy().replace('\\', "/");
    let name = skill.name.as_str();
    let description = skill.description.as_str();
    format!("- {name}: {description} (file: {path_str})")
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
    fn budgeted_rendering_preserves_prompt_priority() {
        let system = make_skill("system-skill", SkillScope::System);
        let user = make_skill("user-skill", SkillScope::User);
        let repo = make_skill("repo-skill", SkillScope::Repo);
        let admin = make_skill("admin-skill", SkillScope::Admin);
        let system_cost = SkillMetadataBudget::Characters(usize::MAX)
            .cost(&format!("{}\n", render_skill_line(&system)));
        let admin_cost = SkillMetadataBudget::Characters(usize::MAX)
            .cost(&format!("{}\n", render_skill_line(&admin)));
        let budget = SkillMetadataBudget::Characters(system_cost + admin_cost);

        let rendered = build_available_skills(
            &[system, user, repo, admin],
            budget,
            SkillRenderSideEffects::None,
        )
        .expect("skills should render");

        assert_eq!(rendered.report.included_count, 2);
        assert_eq!(rendered.report.omitted_count, 2);
        assert!(!rendered.emit_warning);
        let rendered_text = rendered.skill_lines.join("\n");
        assert!(rendered_text.contains("- system-skill:"));
        assert!(rendered_text.contains("- admin-skill:"));
        assert!(!rendered_text.contains("- repo-skill:"));
        assert!(!rendered_text.contains("- user-skill:"));
    }

    #[test]
    fn budgeted_rendering_keeps_scanning_after_oversized_entry() {
        let mut oversized = make_skill("oversized-system-skill", SkillScope::System);
        oversized.description = "desc ".repeat(100);
        let repo = make_skill("repo-skill", SkillScope::Repo);
        let repo_cost = SkillMetadataBudget::Characters(usize::MAX)
            .cost(&format!("{}\n", render_skill_line(&repo)));
        let budget = SkillMetadataBudget::Characters(repo_cost);

        let rendered =
            build_available_skills(&[oversized, repo], budget, SkillRenderSideEffects::None)
                .expect("skills render");

        assert_eq!(rendered.report.included_count, 1);
        assert_eq!(rendered.report.omitted_count, 1);
        assert!(!rendered.emit_warning);
        let rendered_text = rendered.skill_lines.join("\n");
        assert!(!rendered_text.contains("- oversized-system-skill:"));
        assert!(rendered_text.contains("- repo-skill:"));
    }
}
