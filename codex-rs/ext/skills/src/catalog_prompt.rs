use crate::catalog::SkillSourceKind;

const SKILLS_INTRO_WITH_SOURCE_LOCATORS: &str = "A skill is a set of instructions provided through a `SKILL.md` source. Below is the list of skills that can be used. Each entry includes a name, description, and source locator. `file` locators are on the host filesystem, `executor package` locators are owned by their execution environment, `orchestrator package` locators are opaque package identifiers, and `custom resource` locators use their provider's access mechanism.";
const SKILLS_INTRO_WITH_HOST_ALIASES: &str = "A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and a short path that can be expanded into an absolute path using the skill roots table.";
const SKILLS_INTRO_WITH_RESOURCE_ALIASES: &str = "A skill is a set of instructions provided through a `SKILL.md` source. Below is the list of skills that can be used. Each entry includes a name, description, and source locator. Short locators can be expanded using the skill roots table.";
const RESOURCE_ALIAS_INSTRUCTIONS: &str = "- Root aliases: Pass short package locators directly to `skills.read`; it resolves their matching alias from `### Skill roots`.";
const SKILLS_HOW_TO_USE_WITH_SOURCE_LOCATORS: &str = r###"- Discovery: The list above is the skills available in this session (name + description + source locator). `file` entries live on the host filesystem, `executor package` and `orchestrator package` entries are accessed directly through `skills.read`, and `custom resource` entries use their provider's access mechanism.
- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
- Missing/blocked: If a named skill isn't in the list or its source can't be read, say so briefly and continue with the best fallback.
- How to use a skill (progressive disclosure):
  1) After deciding to use a skill, the main agent must read its `SKILL.md` completely before taking task actions. For a `file` entry, open the listed path. For an `executor package` or `orchestrator package`, pass the listed locator directly to `skills.read` as `package`; root aliases are resolved automatically. Omit `resource` to read `SKILL.md` directly without calling `skills.list`. If a read is paginated, follow `next_cursor` until EOF.
  2) When `SKILL.md` references another resource, use the same access mechanism. For executor and orchestrator skills, pass the complete package-contained resource identifier with the same package to `skills.read`; do not treat `skill://` identifiers as filesystem paths.
  3) If `SKILL.md` points to extra folders such as `references/`, use its routing instructions to identify the resources required for the task. The main agent must read each required instruction or reference file itself before acting on it. Do not delegate reading, summarizing, or interpreting skill instructions to a subagent. Subagents may still perform task work when the selected skill allows it.
  4) For filesystem-backed skills, prefer running or patching provided scripts instead of retyping large code blocks. For executor and orchestrator skills, use `skills.read` and the available tools; do not invent a local path.
  5) Reuse provided assets or templates through the same source access mechanism instead of recreating them.
- Coordination and sequencing:
  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.
  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.
- Context hygiene:
  - Progressive disclosure applies to selecting relevant files, not partially reading a selected instruction file. Do not load unrelated references, scripts, or assets.
  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.
  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.
- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue."###;
const SKILLS_HOW_TO_USE_WITH_HOST_ALIASES: &str = r###"- Discovery: The list above is the skills available in this session (name + description + short path). Skill bodies live on disk at the listed paths after expanding the matching alias from `### Skill roots`.
- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.
- How to use a skill (progressive disclosure):
  1) After deciding to use a skill, the main agent must expand the listed short `path` with the matching alias from `### Skill roots`, then open and read its `SKILL.md` completely before taking task actions. If a read is truncated or paginated, continue until EOF.
  2) When `SKILL.md` references relative paths (e.g., `scripts/foo.py`), resolve them relative to the directory containing that expanded `SKILL.md` first, and only consider other paths if needed.
  3) If `SKILL.md` points to extra folders such as `references/`, use its routing instructions to identify the files required for the task. The main agent must read each required instruction or reference file itself before acting on it. Do not delegate reading, summarizing, or interpreting skill instructions to a subagent. Subagents may still perform task work when the selected skill allows it.
  4) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.
  5) If `assets/` or templates exist, reuse them instead of recreating from scratch.
- Coordination and sequencing:
  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.
  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.
- Context hygiene:
  - Progressive disclosure applies to selecting relevant files, not partially reading a selected instruction file. Do not load unrelated references, scripts, or assets.
  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.
  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.
- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue."###;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SkillPromptKind {
    Unaliased,
    HostAliases,
    ResourceAliases,
}

impl SkillPromptKind {
    pub(crate) fn for_aliased_source(source: &SkillSourceKind) -> Self {
        match source {
            SkillSourceKind::Host => Self::HostAliases,
            SkillSourceKind::Executor | SkillSourceKind::Orchestrator => Self::ResourceAliases,
            SkillSourceKind::Custom(_) => Self::Unaliased,
        }
    }

    fn intro(self) -> &'static str {
        match self {
            Self::Unaliased => SKILLS_INTRO_WITH_SOURCE_LOCATORS,
            Self::HostAliases => SKILLS_INTRO_WITH_HOST_ALIASES,
            Self::ResourceAliases => SKILLS_INTRO_WITH_RESOURCE_ALIASES,
        }
    }

    pub(crate) fn usage_instructions(self) -> &'static str {
        match self {
            Self::Unaliased | Self::ResourceAliases => SKILLS_HOW_TO_USE_WITH_SOURCE_LOCATORS,
            Self::HostAliases => SKILLS_HOW_TO_USE_WITH_HOST_ALIASES,
        }
    }

    pub(crate) fn alias_instructions(self) -> Option<&'static str> {
        match self {
            Self::ResourceAliases => Some(RESOURCE_ALIAS_INSTRUCTIONS),
            Self::Unaliased | Self::HostAliases => None,
        }
    }
}

pub(crate) fn render_available_skills_body(
    prompt_kind: SkillPromptKind,
    skill_root_lines: &[String],
    skill_lines: &[String],
) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("## Skills".to_string());
    lines.push(prompt_kind.intro().to_string());
    if !skill_root_lines.is_empty() {
        lines.push("### Skill roots".to_string());
        lines.extend(skill_root_lines.iter().cloned());
    }
    if skill_lines.iter().any(|line| {
        line.contains("(executor package: ") || line.contains("(orchestrator package: ")
    }) {
        lines.push(
            "Read a skill package directly with `skills.read({\"package\":\"<package>\"})` to read its `SKILL.md`; root aliases are resolved automatically. To read another file from that skill, use the same `package` and pass the file's complete `skill://` identifier as `resource`. If the package is not provided, use `skills.list` to find it."
                .to_string(),
        );
    }
    lines.push("### Available skills".to_string());
    lines.extend(skill_lines.iter().cloned());

    format!("\n{}\n", lines.join("\n"))
}
