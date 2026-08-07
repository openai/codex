use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tokio::sync::Semaphore;

use super::catalog_from_outcome;
use crate::loader::HostSkillRoot;
use crate::loader::load_and_merge_host_skill_roots;

#[tokio::test]
async fn host_catalog_entries_carry_their_render_metadata() -> Result<(), Box<dyn std::error::Error>>
{
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
    let outcome = load_and_merge_host_skill_roots(
        vec![HostSkillRoot::host(
            root.clone(),
            SkillScope::User,
            Arc::clone(&LOCAL_FS),
        )],
        &Semaphore::new(/*permits*/ 1),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    let catalog = catalog_from_outcome(&outcome);

    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(
        (
            catalog.entries[0].alias_root(),
            catalog.entries[0].prompt_scope(),
        ),
        (
            Some(root.to_string_lossy().replace('\\', "/").as_str()),
            Some(SkillScope::User),
        )
    );

    std::fs::remove_dir_all(root.as_path())?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn host_catalog_preserves_symlinked_skill_discovery_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let source = tempfile::tempdir()?;
    let source_skill_dir = source.path().join("linked-skill");
    std::fs::create_dir_all(&source_skill_dir)?;
    std::fs::write(
        source_skill_dir.join("SKILL.md"),
        "---\nname: linked-skill\ndescription: Linked skill.\n---\n# Linked skill\n",
    )?;
    std::os::unix::fs::symlink(&source_skill_dir, root.path().join("linked-skill"))?;

    let root = AbsolutePathBuf::try_from(std::fs::canonicalize(root.path())?)?;
    let outcome = load_and_merge_host_skill_roots(
        vec![HostSkillRoot::host(
            root.clone(),
            SkillScope::User,
            Arc::clone(&LOCAL_FS),
        )],
        &Semaphore::new(/*permits*/ 1),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;
    let catalog = catalog_from_outcome(&outcome);
    let canonical_path = std::fs::canonicalize(source_skill_dir.join("SKILL.md"))?;
    let discovery_path = root.join("linked-skill/SKILL.md");

    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(
        (
            catalog.entries[0].main_prompt.as_str(),
            catalog.entries[0].display_path.as_deref(),
            catalog.entries[0].alias_root(),
        ),
        (
            canonical_path.to_string_lossy().as_ref(),
            Some(discovery_path.to_string_lossy().as_ref()),
            Some(root.to_string_lossy().as_ref()),
        )
    );

    Ok(())
}
