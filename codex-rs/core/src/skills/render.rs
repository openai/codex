use crate::skills::model::SkillMetadata;

pub fn render_skills_section(skills: &[SkillMetadata]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push("## Skills".to_string());
    lines.push("These skills are discovered at startup from ~/.codex/skills; each entry shows name, description, and file path so you can open the source for full instructions. Content is not inlined to keep context lean.".to_string());

    for skill in skills {
        let path_str = skill.path.to_string_lossy().replace('\\', "/");
        lines.push(format!(
            "- {}: {} (file: {})",
            skill.name, skill.description, path_str
        ));
    }

    Some(lines.join("\n"))
}
