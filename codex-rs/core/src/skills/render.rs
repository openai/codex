use crate::skills::model::SkillMetadata;

pub fn render_skills_section(skills: &[SkillMetadata]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push("# Skills".to_string());
    lines.push("A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and file path so you can open the source for full instructions when using a specific skill.".to_string());
    lines.push("## Available skills".to_string());

    for skill in skills {
        let path_str = skill.path.to_string_lossy().replace('\\', "/");
        let name = skill.name.as_str();
        let description = skill.description.as_str();
        lines.push(format!("- {name}: {description} (file: {path_str})"));
    }

    lines.push("".to_string());
    lines.push(
        r###"## How to use skills
- Discovery: The list above is the skills available in this session (name + description + file path). Skill bodies live on disk at the listed paths.
- Trigger rules:
  - If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you should do your best to use that skill for that turn. 
  - If the user references multiple skills, you should use them all.
  - Do not carry skills across turns unless re-mentioned.
- Missing/blocked on skill discovery: If a named skill isn't in the list or the path can't be read, pick a reasonable fallback and continue.
- How to use a skill (progressive disclosure):
  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.
  2) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.
  3) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.
  4) If `assets/` or templates exist, reuse them instead of recreating from scratch.
- Coordination and sequencing:
  - If multiple skills apply to the current task, choose the minimal set that can complete the request.
- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), pick the next-best approach, and continue."###
            .to_string(),
    );

    Some(lines.join("\n"))
}
