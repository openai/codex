use std::fs;
use std::time::Duration;

use codex_exec_server::LOCAL_FS;
use codex_skills::EnvironmentSkillMetadata;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use crate::loader::io_test_support::FileSystemCalls;
use crate::loader::io_test_support::ManifestMetadataBehavior;
use crate::loader::io_test_support::RecordingFileSystem;

use super::load_environment_skills_from_root;

#[tokio::test]
async fn loads_nearest_plugin_namespaces_without_reading_unused_sibling_manifests() {
    let root = tempdir().expect("tempdir");
    let standalone_skill = root.path().join("standalone/SKILL.md");
    let outer_root = root.path().join("plugins/outer");
    let outer_skill = outer_root.join("skills/deploy/SKILL.md");
    let inner_root = outer_root.join("nested/inner");
    let inner_skill = inner_root.join("skills/audit/SKILL.md");
    let unused_root = root.path().join("plugins/unused");

    for path in [&standalone_skill, &outer_skill, &inner_skill] {
        fs::create_dir_all(path.parent().expect("skill parent")).expect("skill dir");
    }
    for (plugin_root, name) in [
        (&outer_root, "outer"),
        (&inner_root, "inner"),
        (&unused_root, "unused"),
    ] {
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("manifest dir");
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            format!(r#"{{"name":"{name}"}}"#),
        )
        .expect("manifest");
    }
    for (path, name) in [
        (&standalone_skill, "standalone"),
        (&outer_skill, "deploy"),
        (&inner_skill, "audit"),
    ] {
        fs::write(
            path,
            format!("---\nname: {name}\ndescription: {name} skill.\n---\n"),
        )
        .expect("skill");
    }

    let file_system =
        RecordingFileSystem::new(LOCAL_FS.as_ref(), ManifestMetadataBehavior::Immediate);
    let root_uri = PathUri::from_host_native_path(root.path()).expect("root URI");
    let outcome = load_environment_skills_from_root(
        &file_system,
        &root_uri,
        /*restriction_product*/ None,
    )
    .await;

    assert_eq!(outcome.warnings, Vec::<String>::new());
    assert_eq!(
        outcome.skills,
        vec![
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&inner_skill).unwrap(),
                name: "inner:audit".to_string(),
                description: "audit skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&outer_skill).unwrap(),
                name: "outer:deploy".to_string(),
                description: "deploy skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&standalone_skill).unwrap(),
                name: "standalone".to_string(),
                description: "standalone skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
        ]
    );

    let mut manifest_reads = file_system
        .calls()
        .read_files
        .into_iter()
        .filter(|path| path.basename().as_deref() == Some("plugin.json"))
        .collect::<Vec<_>>();
    manifest_reads.sort_by_key(ToString::to_string);
    let mut expected_manifest_reads = [&outer_root, &inner_root]
        .into_iter()
        .map(|plugin_root| {
            PathUri::from_host_native_path(plugin_root.join(".codex-plugin/plugin.json")).unwrap()
        })
        .collect::<Vec<_>>();
    expected_manifest_reads.sort_by_key(ToString::to_string);
    assert_eq!(manifest_reads, expected_manifest_reads);
}

#[tokio::test]
async fn reuses_walk_inventory_for_missing_skill_metadata() {
    const SKILL_COUNT: usize = 66;

    let root = tempdir().expect("tempdir");
    let manifest_path = root.path().join(".codex-plugin/plugin.json");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent")).expect("manifest dir");
    fs::write(&manifest_path, r#"{"name":"inventory"}"#).expect("manifest");

    let mut skill_paths = Vec::new();
    for index in 0..SKILL_COUNT {
        let name = format!("skill-{index}");
        let skill_path = root.path().join(&name).join("SKILL.md");
        fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("skill dir");
        fs::write(
            &skill_path,
            format!("---\nname: {name}\ndescription: {name} skill.\n---\n"),
        )
        .expect("skill");
        skill_paths.push(skill_path);
    }

    let file_system =
        RecordingFileSystem::new(LOCAL_FS.as_ref(), ManifestMetadataBehavior::Immediate);
    let root_uri = PathUri::from_host_native_path(root.path()).expect("root URI");
    let outcome = load_environment_skills_from_root(
        &file_system,
        &root_uri,
        /*restriction_product*/ None,
    )
    .await;

    let mut expected_skills = skill_paths
        .iter()
        .enumerate()
        .map(|(index, skill_path)| EnvironmentSkillMetadata {
            path_to_skills_md: PathUri::from_host_native_path(skill_path).unwrap(),
            name: format!("inventory:skill-{index}"),
            description: format!("skill-{index} skill."),
            short_description: None,
            dependencies: None,
            policy: None,
        })
        .collect::<Vec<_>>();
    expected_skills.sort_by(|left, right| {
        left.name.cmp(&right.name).then_with(|| {
            left.path_to_skills_md
                .to_string()
                .cmp(&right.path_to_skills_md.to_string())
        })
    });
    assert_eq!(outcome.skills, expected_skills);
    assert_eq!(outcome.warnings, Vec::<String>::new());

    let mut expected_read_files = skill_paths
        .iter()
        .map(|path| PathUri::from_host_native_path(path).unwrap())
        .collect::<Vec<_>>();
    let manifest_uri = PathUri::from_host_native_path(manifest_path).unwrap();
    expected_read_files.push(manifest_uri.clone());
    expected_read_files.sort_by_key(ToString::to_string);
    assert_eq!(
        file_system.calls(),
        FileSystemCalls {
            walks: 1,
            read_files: expected_read_files,
            metadata_files: vec![manifest_uri],
        }
    );
}

#[tokio::test]
async fn reads_skill_files_while_resolving_plugin_namespaces() {
    let root = tempdir().expect("tempdir");
    let manifest_path = root.path().join(".codex-plugin/plugin.json");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent")).expect("manifest dir");
    fs::write(&manifest_path, r#"{"name":"parallel"}"#).expect("manifest");
    let skill_path = root.path().join("demo/SKILL.md");
    fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("skill dir");
    fs::write(
        &skill_path,
        "---\nname: demo\ndescription: demo skill.\n---\n",
    )
    .expect("skill");

    let file_system = RecordingFileSystem::new(
        LOCAL_FS.as_ref(),
        ManifestMetadataBehavior::WaitForSkillRead,
    );
    let root_uri = PathUri::from_host_native_path(root.path()).expect("root URI");
    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        load_environment_skills_from_root(
            &file_system,
            &root_uri,
            /*restriction_product*/ None,
        ),
    )
    .await
    .expect("skill reads should start before namespace resolution finishes");

    assert_eq!(outcome.warnings, Vec::<String>::new());
    assert_eq!(
        outcome.skills,
        vec![EnvironmentSkillMetadata {
            path_to_skills_md: PathUri::from_host_native_path(skill_path).unwrap(),
            name: "parallel:demo".to_string(),
            description: "demo skill.".to_string(),
            short_description: None,
            dependencies: None,
            policy: None,
        }]
    );
}
