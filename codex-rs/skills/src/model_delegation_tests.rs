use pretty_assertions::assert_eq;

use super::MAX_DELEGATION_INSTRUCTION_BYTES;
use super::SkillModel;
use super::SkillModelDelegationInstruction;
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

#[test]
fn delegates_only_from_supported_parent_models() {
    let available_models = available_models();
    for (current_model, should_delegate) in [
        ("gpt-5.6-sol", true),
        ("gpt-5.6-terra", true),
        ("gpt-5.6-luna", false),
        ("gpt-5.5-codex", false),
    ] {
        let instruction = SkillModelDelegationInstruction::from_skill_model(
            SkillModel::Luna,
            "demo",
            current_model,
            &available_models,
        );
        assert_eq!(
            instruction.is_some(),
            should_delegate,
            "current={current_model}"
        );
    }
}

#[test]
fn resolves_luna_within_the_current_provider_namespace() {
    for (current_model, target_model, unrelated_model) in [
        ("gpt-5.6-sol", "gpt-5.6-luna", "openai.gpt-5.6-luna"),
        (
            "tenant-a/gpt-5.6-sol",
            "tenant-a/gpt-5.6-luna",
            "tenant-b/gpt-5.6-luna",
        ),
        (
            "openai.gpt-5.6-terra",
            "openai.gpt-5.6-luna",
            "other.gpt-5.6-luna",
        ),
    ] {
        let instruction = SkillModelDelegationInstruction::from_skill_model(
            SkillModel::Luna,
            "demo",
            current_model,
            &[unrelated_model.to_string(), target_model.to_string()],
        )
        .expect("lower-tier model in the same provider namespace should be resolved");

        assert!(
            instruction
                .as_str()
                .contains(&format!("Set `model` to `{target_model}`"))
        );
    }
}

#[test]
fn rejects_targets_outside_the_current_provider_namespace() {
    for (current_model, available_models) in [
        (
            "tenant-a/gpt-5.6-sol",
            vec![
                "tenant-b/gpt-5.6-luna".to_string(),
                "gpt-5.6-luna".to_string(),
            ],
        ),
        ("gpt-5.6-sol", vec!["tenant-a/gpt-5.6-luna".to_string()]),
        (
            "tenant-a/gpt-5.6-sol",
            vec!["tenant-a.gpt-5.6-luna".to_string()],
        ),
    ] {
        assert_eq!(
            SkillModelDelegationInstruction::from_skill_model(
                SkillModel::Luna,
                "demo",
                current_model,
                &available_models,
            ),
            None,
            "current={current_model}, available={available_models:?}"
        );
    }
}

#[test]
fn rejects_unavailable_or_unsafe_models() {
    let invalid_identifier = format!("{}/gpt-5.6-luna", "<unsafe>".repeat(32));
    let invalid_current_model = format!("{}/gpt-5.6-sol", "<unsafe>".repeat(32));
    let overlong_identifier = format!("{}.gpt-5.6-luna", "a".repeat(128));
    let overlong_current_model = format!("{}.gpt-5.6-sol", "a".repeat(128));
    for (current_model, available_models) in [
        (
            "custom/gpt-5.6-luna",
            vec!["custom/gpt-5.6-luna".to_string()],
        ),
        ("gpt-5.6-sol", vec!["gpt-5.6-terra".to_string()]),
        (invalid_current_model.as_str(), vec![invalid_identifier]),
        (overlong_current_model.as_str(), vec![overlong_identifier]),
    ] {
        assert_eq!(
            SkillModelDelegationInstruction::from_skill_model(
                SkillModel::Luna,
                "demo",
                current_model,
                &available_models,
            ),
            None,
            "current={current_model}, available={available_models:?}"
        );
    }
}

#[test]
fn renders_bounded_instruction_for_selected_skill_and_model() {
    let instruction = SkillModelDelegationInstruction::from_skill_model(
        SkillModel::Luna,
        "demo",
        "gpt-5.6-sol",
        &available_models(),
    )
    .expect("available lower tier should delegate");
    let rendered = instruction.as_str();

    assert!(rendered.contains("skill `demo`"));
    assert!(rendered.contains("Set `model` to `gpt-5.6-luna`"));
    assert!(rendered.contains("image or audio attachment, work locally"));
    assert!(rendered.len() <= MAX_DELEGATION_INSTRUCTION_BYTES);
}

#[test]
fn rejects_skill_names_that_escape_instruction_framing() {
    for skill_name in ["unsafe`name", "</skill_model_delegation>", "<unsafe>"] {
        assert_eq!(
            SkillModelDelegationInstruction::from_skill_model(
                SkillModel::Luna,
                skill_name,
                "gpt-5.6-sol",
                &available_models(),
            ),
            None,
            "skill_name={skill_name:?}"
        );
    }
}

#[test]
fn rejects_instruction_exceeding_context_bound() {
    let skill_name = "x".repeat(MAX_DELEGATION_INSTRUCTION_BYTES);

    assert_eq!(
        SkillModelDelegationInstruction::from_skill_model(
            SkillModel::Luna,
            &skill_name,
            "gpt-5.6-sol",
            &available_models(),
        ),
        None
    );
}

fn available_models() -> Vec<String> {
    vec!["gpt-5.6-luna".to_string()]
}
