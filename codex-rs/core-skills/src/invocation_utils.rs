use std::collections::HashMap;

use crate::SkillMetadata;
use codex_utils_absolute_path::AbsolutePathBuf;

pub(crate) fn build_implicit_skill_path_indexes(
    skills: Vec<SkillMetadata>,
) -> (
    HashMap<AbsolutePathBuf, SkillMetadata>,
    HashMap<AbsolutePathBuf, SkillMetadata>,
) {
    let mut by_scripts_dir = HashMap::new();
    let mut by_skill_doc_path = HashMap::new();
    for skill in skills {
        let skill_doc_path = canonicalize_if_exists(&skill.path_to_skills_md);
        by_skill_doc_path.insert(skill_doc_path, skill.clone());

        if let Some(skill_dir) = skill.path_to_skills_md.parent() {
            let scripts_dir = canonicalize_if_exists(&skill_dir.join("scripts"));
            by_scripts_dir.insert(scripts_dir, skill);
        }
    }

    (by_scripts_dir, by_skill_doc_path)
}

fn canonicalize_if_exists(path: &AbsolutePathBuf) -> AbsolutePathBuf {
    path.canonicalize().unwrap_or_else(|_| path.clone())
}
