use codex_core_skills::SkillLoadOutcome;
use codex_protocol::protocol::SkillScope;
use codex_skills::SkillDependencies;
use codex_skills::SkillInterface;
use codex_skills::SkillMetadata;
use codex_skills::SkillToolDependency;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillResourceId;

use super::*;

#[test]
fn host_interface_metadata_makes_unlisted_workflow_discoverable() {
    let path = AbsolutePathBuf::try_from(std::env::temp_dir().join("routing-card/SKILL.md"))
        .expect("temporary directory should be absolute");
    let resource = path.to_string_lossy().to_string();
    let mut outcome = SkillLoadOutcome::default();
    outcome.skills.push(SkillMetadata {
        name: "content-tools".to_string(),
        description: "Prepare visual content.".to_string(),
        short_description: None,
        interface: Some(SkillInterface {
            display_name: Some("Visual Assets".to_string()),
            short_description: Some("Create illustrated assets.".to_string()),
            icon_small: None,
            icon_large: None,
            brand_color: None,
            default_prompt: Some("Create an animated pet spritesheet.".to_string()),
        }),
        dependencies: None,
        policy: None,
        path_to_skills_md: path,
        scope: SkillScope::User,
        plugin_id: None,
        remote_plugin_id: None,
    });
    let snapshot = HostSkillsSnapshot::new(std::sync::Arc::new(outcome));
    let catalog = SkillCatalog {
        entries: vec![SkillCatalogEntry::new(
            SkillPackageId(resource.clone()),
            SkillAuthority::new(SkillSourceKind::Host, "host"),
            "content-tools",
            "Prepare visual content.",
            SkillResourceId::new(resource),
        )],
        ..Default::default()
    };
    let selector = CharacterRoutingCardSkillSelector::new(&catalog, Some(&snapshot));
    let documents = [document(
        /*id*/ 0,
        "content-tools",
        "Prepare visual content.",
    )];

    let selection = selector.select(
        "make an animated pet spritesheet",
        &documents,
        /*limit*/ 20,
    );

    assert_eq!(vec![0], selection.candidate_ids);
}

#[test]
fn connector_dependencies_make_generic_skills_discoverable() {
    let dependencies = SkillDependencies {
        tools: vec![SkillToolDependency {
            r#type: "app".to_string(),
            value: "slack".to_string(),
            description: Some("Post messages to team channels.".to_string()),
            transport: None,
            command: None,
            url: None,
        }],
    };
    let documents = [
        SkillSelectionDocument {
            dependencies: Some(&dependencies),
            ..document(
                /*id*/ 3,
                "team-communication",
                "Share updates with the team.",
            )
        },
        document(/*id*/ 4, "documents", "Write project updates."),
    ];
    let selector = CharacterRoutingCardSkillSelector::new(
        &SkillCatalog::default(),
        /*host_snapshot*/ None,
    );

    let selection = selector.select("post this update in Slack", &documents, /*limit*/ 20);

    assert_eq!(Some(3), selection.candidate_ids.first().copied());
}

#[test]
fn strong_baseline_match_is_preserved_alongside_routing_match() {
    let documents = [
        document(/*id*/ 1, "presentation-tools", "Build meeting decks."),
        document(/*id*/ 2, "content-tools", "Prepare visual content."),
    ];
    let selector = CharacterRoutingCardSkillSelector {
        routing_fields: HashMap::from([(
            2,
            "Create a presentation with PowerPoint slides".to_string(),
        )]),
    };

    let selection = selector.select(
        "create a PowerPoint presentation",
        &documents,
        /*limit*/ 2,
    );

    assert_eq!(vec![2, 1], selection.candidate_ids);
}

#[test]
fn long_routing_metadata_preserves_original_short_description_matches() {
    let metadata = (0_u64..60)
        .map(|index| format!("{:016x}", index.wrapping_mul(/*rhs*/ 0x9e37_79b9_7f4a_7c15)))
        .collect::<Vec<_>>()
        .join(" ");
    let short_description = "zebra xylophone quasar";
    let documents = [SkillSelectionDocument {
        short_description: Some(short_description),
        ..document(/*id*/ 0, "content-tools", "Prepare visual content.")
    }];
    let selector = CharacterRoutingCardSkillSelector {
        routing_fields: HashMap::from([(/*id*/ 0, metadata)]),
    };

    let selection = selector.select(short_description, &documents, /*limit*/ 20);

    assert_eq!(vec![0], selection.candidate_ids);
}

fn document<'a>(id: usize, name: &'a str, description: &'a str) -> SkillSelectionDocument<'a> {
    SkillSelectionDocument {
        id,
        name,
        short_description: None,
        description,
        dependencies: None,
    }
}
