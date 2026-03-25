use crate::SkillMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDependencyInfo {
    pub skill_name: String,
    pub name: String,
    pub description: Option<String>,
}

pub fn collect_env_var_dependencies(
    mentioned_skills: &[SkillMetadata],
) -> Vec<SkillDependencyInfo> {
    let mut dependencies = Vec::new();
    for skill in mentioned_skills {
        let Some(skill_dependencies) = &skill.dependencies else {
            continue;
        };
        for tool in &skill_dependencies.tools {
            if tool.r#type != "env_var" || tool.value.is_empty() {
                continue;
            }
            dependencies.push(SkillDependencyInfo {
                skill_name: skill.name.clone(),
                name: tool.value.clone(),
                description: tool.description.clone(),
            });
        }
    }
    dependencies
}
