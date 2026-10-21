//! Exercises managed storage projection, validation, and cloud layer precedence.

use crate::ConfigRequirementsToml;
use crate::config_toml::ConfigToml;
use pretty_assertions::assert_eq;
use serde_json::json;

/// Managed paths override local preferences without replacing unrelated desktop settings.
#[test]
fn storage_requirements_project_and_protect_only_managed_settings() {
    let home = tempfile::tempdir().unwrap();
    let parent = home.path().join("projects");
    let scratch = home.path().join("scratch");
    let requirements: ConfigRequirementsToml = toml::from_str(&format!(
        "[application.storage]\ndefault_project_parent = '{}'\nprojectless_workspace_root = '{}'\n",
        parent.display(),
        scratch.display()
    ))
    .unwrap();
    let mut config: ConfigToml = toml::from_str(
        "[desktop]\ndefaultProjectParent = 'local'\nprojectlessWorkspaceRoot = 'local'\nlocaleOverride = 'fr-FR'"
    ).unwrap();
    requirements.apply_exact_to_config(&mut config);
    assert_eq!(
        serde_json::to_value(config.desktop).unwrap(),
        json!({
            "defaultProjectParent": parent,
            "projectlessWorkspaceRoot": scratch,
            "localeOverride": "fr-FR"
        })
    );
    for (segments, expected) in [
        (vec![], Some("desktop.defaultProjectParent")),
        (vec!["desktop"], Some("desktop.defaultProjectParent")),
        (
            vec!["desktop", "defaultProjectParent"],
            Some("desktop.defaultProjectParent"),
        ),
        (
            vec!["desktop", "projectlessWorkspaceRoot"],
            Some("desktop.projectlessWorkspaceRoot"),
        ),
        (vec!["desktop", "localeOverride"], None),
        (vec!["model"], None),
    ] {
        let segments = segments.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            requirements.exact_requirement_for_config_path(&segments),
            expected
        );
    }
    assert!(!requirements.is_empty());
}

/// Invalid managed paths must fail parsing rather than silently selecting Documents.
#[test]
fn storage_requirements_reject_relative_paths_and_non_path_values() {
    for contents in [
        "[application.storage]\ndefault_project_parent = 'relative'",
        "[application.storage]\nprojectless_workspace_root = 'relative'",
        "[application.storage]\ndefault_project_parent = false",
        "[application.storage]\ndefault_project_parent = '${HOME}/Codex'",
    ] {
        assert!(toml::from_str::<ConfigRequirementsToml>(contents).is_err());
    }
}

/// Misspelled policy keys must not silently leave storage unmanaged.
#[test]
fn storage_requirements_reject_unknown_keys() {
    for key in ["default_project_parrent", "projectless_workspace_rot"] {
        let contents = format!("[application.storage]\n{key} = 'ignored'");
        let error = toml::from_str::<ConfigRequirementsToml>(&contents).unwrap_err();
        assert!(error.message().contains(&format!("unknown field `{key}`")));
    }
}

/// Cloud fragments merge per setting without losing independent fields from lower-priority layers.
#[test]
fn cloud_storage_layers_preserve_independent_fields_and_precedence() {
    use crate::AbsolutePathBuf;
    use crate::CloudConfigBundle;
    use crate::CloudConfigBundleLayers;
    use crate::CloudRequirementsFragment;
    use crate::RequirementSource;
    use crate::RequirementsLayerEntry;
    use crate::compose_requirements;
    let home = tempfile::tempdir().unwrap();
    let base = AbsolutePathBuf::from_absolute_path(home.path()).unwrap();
    let projects = home.path().join("projects");
    let chats = home.path().join("chats");
    let local = home.path().join("local");
    let mut bundle = CloudConfigBundle::default();
    bundle.requirements_toml.enterprise_managed = vec![
        CloudRequirementsFragment {
            id: "high".into(),
            name: "Projects".into(),
            contents: format!(
                "[application.network]\nenabled = false\n[application.storage]\ndefault_project_parent = '{}'",
                projects.display()
            ),
        },
        CloudRequirementsFragment {
            id: "low".into(),
            name: "Storage".into(),
            contents: format!(
                "[application.storage]\ndefault_project_parent = '{}'\nprojectless_workspace_root = '{}'",
                home.path().display(),
                chats.display()
            ),
        },
    ];
    let mut layers = CloudConfigBundleLayers::from_bundle(bundle, &base)
        .unwrap()
        .enterprise_managed_requirements;
    let requirements = compose_requirements(layers.clone())
        .unwrap()
        .unwrap()
        .into_toml();
    assert!(
        !requirements
            .application
            .as_ref()
            .unwrap()
            .network
            .as_ref()
            .unwrap()
            .enabled
    );
    assert_eq!(
        requirements
            .application
            .unwrap()
            .storage
            .map(|storage| *storage),
        Some(super::ApplicationStorageRequirementsToml {
            default_project_parent: Some(AbsolutePathBuf::from_absolute_path(&projects).unwrap()),
            projectless_workspace_root: Some(AbsolutePathBuf::from_absolute_path(&chats).unwrap()),
        })
    );
    layers.push(RequirementsLayerEntry::from_toml(
        RequirementSource::Unknown,
        format!(
            "[application.storage]\ndefault_project_parent = '{}'",
            local.display()
        ),
    ));
    let requirements = compose_requirements(layers).unwrap().unwrap().into_toml();
    assert!(
        !requirements
            .application
            .as_ref()
            .unwrap()
            .network
            .as_ref()
            .unwrap()
            .enabled
    );
    assert_eq!(
        requirements
            .application
            .unwrap()
            .storage
            .map(|storage| *storage),
        Some(super::ApplicationStorageRequirementsToml {
            default_project_parent: Some(AbsolutePathBuf::from_absolute_path(local).unwrap()),
            projectless_workspace_root: Some(AbsolutePathBuf::from_absolute_path(chats).unwrap()),
        })
    );
}
