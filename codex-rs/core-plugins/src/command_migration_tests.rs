use super::*;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;

const TEST_REWRITE_PROFILE: RewriteProfile = RewriteProfile::new(
    "CLAUDE.md",
    &[
        "claude code",
        "claude-code",
        "claude_code",
        "claudecode",
        "claude",
    ],
);

#[test]
fn command_skill_names_must_fit_codex_skill_loader_limit() {
    let source_name = "this-is-a-deeply-nested-command-with-a-very-long-name";
    let file = Path::new("commands/this/is/a/deeply/nested/command/with/a/very/long/name.md");
    let document = parse_command_content("---\ndescription: Review PR\n---\nReview\n");

    assert!(
        command_skill_name_if_supported(
            source_name,
            file,
            &document,
            CommandDescriptionMode::RequireFrontmatter,
        )
        .is_none()
    );
}

#[test]
fn commands_with_overlong_descriptions_are_preserved() {
    let description = "x".repeat(1025);
    let document =
        parse_command_content(&format!("---\ndescription: {description}\n---\nReview\n"));

    assert_eq!(
        command_skill_name_if_supported(
            "review",
            Path::new("commands/review.md"),
            &document,
            CommandDescriptionMode::RequireFrontmatter,
        ),
        Some("source-command-review".to_string())
    );

    let rendered = render_command_skill(
        &document.body,
        "source-command-review",
        &description,
        "review",
        TEST_REWRITE_PROFILE,
    );
    assert_eq!(
        parse_command_content(&rendered).description.as_deref(),
        Some(description.as_str())
    );
}

#[test]
fn commands_with_provider_runtime_expansion_are_skipped() {
    let document = parse_command_content(
        "---\ndescription: Deploy\n---\nDeploy $ARGUMENTS from @release.yaml\n",
    );

    assert!(
        command_skill_name_if_supported(
            "deploy",
            Path::new("commands/deploy.md"),
            &document,
            CommandDescriptionMode::RequireFrontmatter,
        )
        .is_none()
    );
}

#[test]
fn commands_without_description_are_skipped() {
    let document = parse_command_content("Review the current change.\n");

    assert!(
        command_skill_name_if_supported(
            "review",
            Path::new("commands/review.md"),
            &document,
            CommandDescriptionMode::RequireFrontmatter,
        )
        .is_none()
    );
}

#[test]
fn commands_can_derive_descriptions_from_source_names() {
    let root = tempfile::TempDir::new().expect("tempdir");
    let commands = root.path().join("commands");
    let target_skills = root.path().join("skills");
    fs::create_dir_all(&commands).expect("create commands");
    fs::write(
        commands.join("review-code.md"),
        "Review the current change.\n",
    )
    .expect("write command");
    let profile = CommandMigrationProfile::new(
        TEST_REWRITE_PROFILE,
        CommandDescriptionMode::UseSourceNameFallback,
    );

    assert_eq!(
        import_commands_with_profile(&commands, &target_skills, profile).unwrap(),
        vec!["source-command-review-code".to_string()]
    );
    let rendered = fs::read_to_string(
        target_skills
            .join("source-command-review-code")
            .join("SKILL.md"),
    )
    .expect("read migrated command");
    assert!(rendered.contains("description: \"Migrated source command `review-code`\""));
    assert!(rendered.contains("Review the current change."));
}

#[test]
fn command_slug_collisions_are_skipped() {
    let root = tempfile::TempDir::new().expect("tempdir");
    let commands = root.path().join("commands");
    fs::create_dir_all(&commands).expect("create commands");
    fs::write(
        commands.join("foo-bar.md"),
        "---\ndescription: First\n---\nRun the first command.\n",
    )
    .expect("write first command");
    fs::write(
        commands.join("foo_bar.md"),
        "---\ndescription: Second\n---\nRun the second command.\n",
    )
    .expect("write second command");

    assert_eq!(
        unique_supported_command_sources(&commands, CommandDescriptionMode::RequireFrontmatter,)
            .unwrap(),
        Vec::<CommandSource>::new()
    );
}
