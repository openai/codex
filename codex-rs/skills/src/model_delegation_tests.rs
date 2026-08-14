use pretty_assertions::assert_eq;

use super::SkillModel;
use crate::ParsedSkillFrontmatter;
use crate::parse_skill_frontmatter_metadata;

#[test]
fn parses_supported_luna_skill_model() {
    let contents = "---\nname: demo\ndescription: Demo skill\nmodel: luna\n---\n";

    assert_eq!(
        parse_skill_frontmatter_metadata(contents, || "fallback".to_string())
            .unwrap()
            .model,
        Some(SkillModel::Luna)
    );
}

#[test]
fn permits_skills_without_a_model_annotation() {
    let contents = "---\nname: demo\ndescription: Demo skill\n---\n";

    assert_eq!(
        parse_skill_frontmatter_metadata(contents, || "fallback".to_string())
            .unwrap()
            .model,
        None
    );
}

#[test]
fn rejects_unsupported_model_without_disabling_ordinary_skill_loading() {
    for unsupported_model in ["terra", "sol", "unknown"] {
        let contents =
            format!("---\nname: demo\ndescription: Demo skill\nmodel: {unsupported_model}\n---\n");

        assert_eq!(
            parse_skill_frontmatter_metadata(&contents, || "fallback".to_string()).unwrap(),
            ParsedSkillFrontmatter {
                name: "demo".to_string(),
                description: "Demo skill".to_string(),
                short_description: None,
                model: None,
            }
        );
    }
}

#[test]
fn reuses_existing_frontmatter_scalar_repair() {
    let contents = "---\nname: demo\ndescription: Build for AWS: ECS\nmodel: luna\n---\n";

    assert_eq!(
        parse_skill_frontmatter_metadata(contents, || "fallback".to_string())
            .unwrap()
            .model,
        Some(SkillModel::Luna)
    );
}
