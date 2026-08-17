use std::fs;
use std::os::unix::fs::symlink;
use std::sync::Arc;

use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::SkillScope;
use codex_skills::SkillMetadata;
use codex_skills::SkillPolicy;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use tokio::sync::Semaphore;

use crate::loader::HostSkillRoot;
use crate::loader::MAX_CONCURRENT_ROOT_SCANS;
use crate::loader::io_test_support::ManifestMetadataBehavior;
use crate::loader::io_test_support::RecordingFileSystem;
use crate::loader::load_and_merge_host_skill_roots;

#[tokio::test]
async fn host_loading_reuses_walk_inventory_for_symlinked_skill_pack() {
    let root = tempdir().expect("tempdir");
    let shared_plugin_root = tempdir().expect("tempdir");
    let manifest_path = shared_plugin_root.path().join(".codex-plugin/plugin.json");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent")).expect("manifest dir");
    fs::write(&manifest_path, r#"{"name":"linked"}"#).expect("manifest");

    let skills_root = shared_plugin_root.path().join("skills");
    for name in ["first", "second"] {
        let skill_path = skills_root.join(name).join("SKILL.md");
        fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("skill dir");
        fs::write(
            &skill_path,
            format!("---\nname: {name}\ndescription: {name} skill.\n---\n"),
        )
        .expect("skill");
    }
    let metadata_path = skills_root.join("first/agents/openai.yaml");
    fs::create_dir_all(metadata_path.parent().expect("metadata parent")).expect("metadata dir");
    fs::write(
        &metadata_path,
        "policy:\n  allow_implicit_invocation: false\n",
    )
    .expect("metadata");

    let host_root = root.path().join("skills");
    fs::create_dir_all(&host_root).expect("host skills dir");
    let linked_root = host_root.join("linked-plugin");
    symlink(&skills_root, &linked_root).expect("skill pack symlink");

    let recording = Arc::new(RecordingFileSystem::new(
        LOCAL_FS.as_ref(),
        ManifestMetadataBehavior::Immediate,
    ));
    let file_system: Arc<dyn ExecutorFileSystem> = recording.clone();
    let root_scan_slots = Semaphore::new(MAX_CONCURRENT_ROOT_SCANS);
    let future = load_and_merge_host_skill_roots(
        vec![HostSkillRoot::host(
            host_root.abs(),
            SkillScope::User,
            file_system,
        )],
        &root_scan_slots,
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    );
    fn assert_send<T: Send>(_: &T) {}
    assert_send(&future);
    let outcome = future.await;

    assert_eq!(outcome.errors, Vec::new());
    let first_skill_path = dunce::canonicalize(skills_root.join("first/SKILL.md"))
        .unwrap()
        .abs();
    let second_skill_path = dunce::canonicalize(skills_root.join("second/SKILL.md"))
        .unwrap()
        .abs();
    assert_eq!(
        outcome.skills,
        vec![
            SkillMetadata {
                name: "linked:first".to_string(),
                description: "first skill.".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: Some(SkillPolicy {
                    allow_implicit_invocation: Some(false),
                    products: Vec::new(),
                }),
                path_to_skills_md: first_skill_path,
                scope: SkillScope::User,
                plugin_id: None,
                remote_plugin_id: None,
            },
            SkillMetadata {
                name: "linked:second".to_string(),
                description: "second skill.".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                path_to_skills_md: second_skill_path,
                scope: SkillScope::User,
                plugin_id: None,
                remote_plugin_id: None,
            },
        ]
    );

    let calls = recording.calls();
    assert_eq!(calls.walks, 1);
    let linked_root = PathUri::from_host_native_path(linked_root).unwrap();
    assert!(
        calls
            .read_files
            .iter()
            .all(|path| !path.starts_with(&linked_root))
    );
    assert!(
        calls
            .metadata_files
            .iter()
            .all(|path| path.basename().as_deref() != Some("openai.yaml"))
    );
    let manifest_uri =
        PathUri::from_host_native_path(dunce::canonicalize(manifest_path).unwrap()).unwrap();
    assert_eq!(
        calls
            .metadata_files
            .iter()
            .filter(|path| **path == manifest_uri)
            .count(),
        1
    );
}
