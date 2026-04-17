use crate::model::SkillMetadata;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::SkillScope;
use codex_utils_output_truncation::approx_token_count;

pub const DEFAULT_SKILL_METADATA_CHAR_BUDGET: usize = 8_000;
pub const SKILL_METADATA_CONTEXT_WINDOW_PERCENT: usize = 2;
const DEFAULT_OMITTED_SKILL_SAMPLE_LIMIT: usize = 5;

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

    fn describe(self) -> String {
        match self {
            Self::Tokens(limit) => format!("{limit} tokens"),
            Self::Characters(limit) => format!("{limit} characters"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmittedSkillSummary {
    pub name: String,
    pub scope: SkillScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRenderReport {
    pub budget: SkillMetadataBudget,
    pub total_count: usize,
    pub included_count: usize,
    pub omitted_count: usize,
    pub omitted_samples: Vec<OmittedSkillSummary>,
}

impl SkillRenderReport {
    pub fn warning_message(&self) -> Option<String> {
        if self.omitted_count == 0 {
            return None;
        }

        let omitted = if self.omitted_samples.is_empty() {
            "some skills".to_string()
        } else {
            let mut names = self
                .omitted_samples
                .iter()
                .map(|skill| format!("{} ({})", skill.name, scope_label(skill.scope)))
                .collect::<Vec<_>>();
            let hidden_count = self.omitted_count.saturating_sub(names.len());
            if hidden_count > 0 {
                names.push(format!("{hidden_count} more"));
            }
            names.join(", ")
        };

        Some(format!(
            "Skills list trimmed to fit the metadata budget: showing {included} of {total} enabled skills ({budget}). Omitted skills include {omitted}. Explicitly mention a skill by name or path if needed, or disable unused skills to reduce the list.",
            included = self.included_count,
            total = self.total_count,
            budget = self.budget.describe(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSkillsSection {
    pub text: String,
    pub report: SkillRenderReport,
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

pub fn render_skills_section(skills: &[SkillMetadata]) -> Option<String> {
    render_skills_section_inner(skills, None).map(|rendered| rendered.text)
}

pub fn render_skills_section_with_budget(
    skills: &[SkillMetadata],
    budget: SkillMetadataBudget,
) -> Option<RenderedSkillsSection> {
    render_skills_section_inner(skills, Some(budget))
}

fn render_skills_section_inner(
    skills: &[SkillMetadata],
    budget: Option<SkillMetadataBudget>,
) -> Option<RenderedSkillsSection> {
    if skills.is_empty() {
        return None;
    }

    let (skill_lines, report) = render_skill_lines(skills, budget);
    let mut lines: Vec<String> = Vec::new();
    lines.push("## Skills".to_string());
    lines.push("A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and file path so you can open the source for full instructions when using a specific skill.".to_string());
    lines.push("### Available skills".to_string());
    if skill_lines.is_empty() {
        lines.push("No skill metadata entries fit within the configured budget.".to_string());
    } else {
        lines.extend(skill_lines);
    }
    if let Some(message) = report.warning_message() {
        lines.push(message);
    }

    lines.push("### How to use skills".to_string());
    lines.push(
        r###"- Discovery: The list above is the skills available in this session (name + description + file path). Skill bodies live on disk at the listed paths.
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
- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue."###
            .to_string(),
    );

    let body = lines.join("\n");
    Some(RenderedSkillsSection {
        text: format!("{SKILLS_INSTRUCTIONS_OPEN_TAG}\n{body}\n{SKILLS_INSTRUCTIONS_CLOSE_TAG}"),
        report,
    })
}

fn render_skill_lines(
    skills: &[SkillMetadata],
    budget: Option<SkillMetadataBudget>,
) -> (Vec<String>, SkillRenderReport) {
    let ordered_skills = ordered_skills_for_budget(skills, budget.is_some());
    let Some(budget) = budget else {
        return (
            ordered_skills
                .iter()
                .map(|skill| render_skill_line(skill))
                .collect(),
            SkillRenderReport {
                budget: SkillMetadataBudget::Characters(usize::MAX),
                total_count: skills.len(),
                included_count: skills.len(),
                omitted_count: 0,
                omitted_samples: Vec::new(),
            },
        );
    };

    let mut included = Vec::new();
    let mut omitted_samples = Vec::new();
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
        if omitted_samples.len() < DEFAULT_OMITTED_SKILL_SAMPLE_LIMIT {
            omitted_samples.push(OmittedSkillSummary {
                name: skill.name.clone(),
                scope: skill.scope,
            });
        }
    }

    let report = SkillRenderReport {
        budget,
        total_count: skills.len(),
        included_count: included.len(),
        omitted_count,
        omitted_samples,
    };

    (included, report)
}

fn ordered_skills_for_budget(
    skills: &[SkillMetadata],
    prioritize_for_budget: bool,
) -> Vec<&SkillMetadata> {
    let mut ordered = skills.iter().collect::<Vec<_>>();
    if prioritize_for_budget {
        ordered.sort_by(|a, b| {
            prompt_scope_rank(a.scope)
                .cmp(&prompt_scope_rank(b.scope))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.path_to_skills_md.cmp(&b.path_to_skills_md))
        });
    }
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

fn scope_label(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::Admin => "admin",
        SkillScope::Repo => "repo",
        SkillScope::User => "user",
        SkillScope::System => "system",
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
            default_skill_metadata_budget(None),
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

        let rendered = render_skills_section_with_budget(&[system, user, repo, admin], budget)
            .expect("skills should render");

        assert_eq!(rendered.report.included_count, 2);
        assert_eq!(rendered.report.omitted_count, 2);
        assert!(rendered.text.contains("- system-skill:"));
        assert!(rendered.text.contains("- admin-skill:"));
        assert!(!rendered.text.contains("- repo-skill:"));
        assert!(!rendered.text.contains("- user-skill:"));
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
            render_skills_section_with_budget(&[oversized, repo], budget).expect("skills render");

        assert_eq!(rendered.report.included_count, 1);
        assert_eq!(rendered.report.omitted_count, 1);
        assert!(!rendered.text.contains("- oversized-system-skill:"));
        assert!(rendered.text.contains("- repo-skill:"));
    }

    #[test]
    fn unbudgeted_rendering_preserves_input_order() {
        let user = make_skill("user-skill", SkillScope::User);
        let admin = make_skill("admin-skill", SkillScope::Admin);

        let rendered = render_skills_section(&[user, admin]).expect("skills should render");

        let user_index = rendered.find("- user-skill:").expect("user skill");
        let admin_index = rendered.find("- admin-skill:").expect("admin skill");
        assert!(user_index < admin_index);
    }
}
