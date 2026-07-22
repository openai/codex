use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_core_skills::loader::SkillRoot;
use codex_core_skills::loader::load_skills_from_roots;
use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tokio::sync::Semaphore;

use super::catalog_from_outcome;

#[tokio::test]
async fn host_catalog_entries_carry_their_prompt_scope() -> Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "codex-skills-extension-host-provider-{}-{unique}",
        std::process::id()
    ));
    let skill_path = root.join("demo").join("SKILL.md");
    std::fs::create_dir_all(
        skill_path
            .parent()
            .ok_or("skill path should have a parent")?,
    )?;
    std::fs::write(
        &skill_path,
        "---\nname: demo\ndescription: Demo skill.\n---\n# Demo\n",
    )?;
    let root = AbsolutePathBuf::try_from(std::fs::canonicalize(root)?)?;
    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: root.clone(),
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_id: None,
            plugin_namespace: None,
            plugin_root: None,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(1)),
    )
    .await;

    let catalog = catalog_from_outcome(&outcome);

    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].prompt_scope(), Some(SkillScope::User));

    std::fs::remove_dir_all(root.as_path())?;
    Ok(())
}
