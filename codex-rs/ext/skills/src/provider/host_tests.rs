use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tokio::sync::Semaphore;

use super::HostSkillProvider;
use super::catalog_from_outcome;
use crate::HostSkillsSnapshot;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::loader::HostSkillRoot;
use crate::loader::load_and_merge_host_skill_roots;
use crate::provider::SkillProvider;
use crate::provider::SkillReadContext;
use crate::provider::SkillReadRequest;

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

/// Host catalog IDs and displayed paths must resolve to the same discovered file,
/// including Windows slash spelling and characters that need URI escaping.
#[tokio::test]
async fn host_catalog_resource_spellings_read_the_discovered_skill()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let skill_path = root.path().join("demo skill%23").join("SKILL.md");
    std::fs::create_dir_all(
        skill_path
            .parent()
            .ok_or("skill path should have a parent")?,
    )?;
    let contents = "---\nname: demo\ndescription: Demo skill.\n---\n# Demo\n";
    std::fs::write(&skill_path, contents)?;
    let root_path = AbsolutePathBuf::try_from(std::fs::canonicalize(root.path())?)?;
    let outcome = load_and_merge_host_skill_roots(
        vec![HostSkillRoot::host(
            root_path,
            SkillScope::User,
            Arc::clone(&LOCAL_FS),
        )],
        &Semaphore::new(/*permits*/ 1),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;
    let catalog = catalog_from_outcome(&outcome);
    let entry = catalog
        .entries
        .first()
        .ok_or("skill should be discovered")?;
    let snapshot = Arc::new(HostSkillsSnapshot::new(Arc::new(outcome)));
    for resource in [
        entry.main_prompt.clone(),
        SkillResourceId::new(entry.display_path.as_ref().ok_or("skill display path")?),
    ] {
        let result = HostSkillProvider::new()
            .read(SkillReadRequest {
                authority: entry.authority.clone(),
                package: entry.id.clone(),
                resource: resource.clone(),
                context: SkillReadContext::Host {
                    host_snapshot: Some(Arc::clone(&snapshot)),
                },
            })
            .await?;
        assert_eq!(
            result,
            SkillReadResult {
                resource,
                contents: contents.to_string()
            }
        );
    }
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
