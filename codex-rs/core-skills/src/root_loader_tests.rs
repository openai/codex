use std::fs;
use std::sync::Arc;

use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::test_support::PathExt;

use super::SkillRootLoader;
use crate::loader::SkillRoot;

fn plugin_root(path: &std::path::Path) -> SkillRoot {
    SkillRoot {
        path: path.join("skills").abs(),
        scope: SkillScope::User,
        file_system: Arc::clone(&LOCAL_FS),
        plugin_id: Some("sample@test".to_string()),
        plugin_namespace: Some("sample".to_string()),
        plugin_root: Some(path.abs()),
    }
}

fn write_skill(path: &std::path::Path, description: &str) {
    let skill_dir = path.join("skills/search");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: search\ndescription: {description}\n---\n"),
    )
    .expect("write skill");
}

#[tokio::test]
async fn reuses_plugin_root_snapshot_until_cache_is_cleared() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let loader = SkillRootLoader::default();

    write_skill(tempdir.path(), "first");
    let first = loader
        .load_skills_from_roots([plugin_root(tempdir.path())])
        .await;

    write_skill(tempdir.path(), "second");
    let cached = loader
        .load_skills_from_roots([plugin_root(tempdir.path())])
        .await;
    assert_eq!(cached.skills, first.skills);

    loader.clear_cache();
    let refreshed = loader
        .load_skills_from_roots([plugin_root(tempdir.path())])
        .await;
    let mut expected = first.skills;
    expected[0].description = "second".to_string();
    assert_eq!(refreshed.skills, expected);
}
