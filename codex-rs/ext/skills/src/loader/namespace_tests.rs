use std::collections::HashSet;
use std::fs;
use std::path::Path;
#[cfg(unix)]
use std::sync::Arc;

use codex_exec_server::LOCAL_FS;
#[cfg(unix)]
use codex_protocol::protocol::SkillScope;
#[cfg(unix)]
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[cfg(unix)]
use crate::loader::HostSkillRoot;
#[cfg(unix)]
use crate::loader::load_host_skill_root;

use super::SkillNamespaceResolver;

fn canonical_uri(path: &Path) -> PathUri {
    PathUri::from_host_native_path(fs::canonicalize(path).expect("canonical path"))
        .expect("absolute path URI")
}

fn write_skill(root: &Path, directory: &str) -> PathUri {
    let path = root.join(directory).join("SKILL.md");
    fs::create_dir_all(path.parent().expect("skill parent")).expect("create skill directory");
    fs::write(&path, "---\nname: skill\ndescription: Demo skill\n---\n")
        .expect("write skill frontmatter");
    canonical_uri(&path)
}

fn write_manifest(root: &Path, contents: &str) -> PathUri {
    let path = root.join(".codex-plugin/plugin.json");
    fs::create_dir_all(path.parent().expect("manifest parent"))
        .expect("create plugin manifest directory");
    fs::write(path, contents).expect("write plugin manifest");
    canonical_uri(root)
}

async fn namespaces_for(
    root: &Path,
    skill_paths: &[PathUri],
    plugin_roots: HashSet<PathUri>,
    mut namespace_roots: HashSet<PathUri>,
) -> Vec<String> {
    let root = canonical_uri(root);
    namespace_roots.insert(root.clone());
    let resolver = SkillNamespaceResolver::discover(
        LOCAL_FS.as_ref(),
        &root,
        skill_paths,
        plugin_roots,
        namespace_roots,
    )
    .await;

    skill_paths
        .iter()
        .map(|path| resolver.for_skill(&root, path).qualify("skill"))
        .collect()
}

#[tokio::test]
async fn nested_plugin_namespaces_do_not_apply_to_plain_siblings() {
    let root = TempDir::new().expect("root temp dir");
    let nested_plugin = root.path().join("nested-plugin");
    let plugin_root = write_manifest(&nested_plugin, r#"{"name":"nested"}"#);
    let plain_skill = write_skill(root.path(), "plain");
    let nested_skill = write_skill(&nested_plugin, "skills/search");

    let namespaces = namespaces_for(
        root.path(),
        &[plain_skill, nested_skill],
        HashSet::from([plugin_root]),
        HashSet::new(),
    )
    .await;

    assert_eq!(namespaces, vec!["skill", "nested:skill"]);
}

#[tokio::test]
async fn nearest_valid_manifest_overrides_or_falls_back_to_outer_namespace() {
    let root = TempDir::new().expect("root temp dir");
    let plugin_root = root.path().join("outer");
    let skills_root = plugin_root.join("skills");
    let nested_plugin = skills_root.join("nested");
    let invalid_plugin = skills_root.join("invalid");
    write_manifest(&plugin_root, r#"{"name":"outer"}"#);
    let nested_root = write_manifest(&nested_plugin, r#"{"name":"nested"}"#);
    let invalid_root = write_manifest(&invalid_plugin, "not json");
    let direct_skill = write_skill(&skills_root, "direct");
    let nested_skill = write_skill(&nested_plugin, "skills/search");
    let fallback_skill = write_skill(&invalid_plugin, "skills/fallback");

    let namespaces = namespaces_for(
        &skills_root,
        &[direct_skill, nested_skill, fallback_skill],
        HashSet::from([nested_root, invalid_root]),
        HashSet::new(),
    )
    .await;

    assert_eq!(
        namespaces,
        vec!["outer:skill", "nested:skill", "outer:skill"]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlinked_external_directories_do_not_inherit_plugin_namespace() {
    let root = TempDir::new().expect("root temp dir");
    let external = TempDir::new().expect("external temp dir");
    let plugin_root = root.path().join("outer");
    let skills_root = plugin_root.join("skills");
    write_manifest(&plugin_root, r#"{"name":"outer"}"#);
    write_skill(external.path(), "plain");
    fs::create_dir_all(&skills_root).expect("create skill root");
    std::os::unix::fs::symlink(external.path(), skills_root.join("linked"))
        .expect("create external directory symlink");

    let snapshot = load_host_skill_root(HostSkillRoot::host(
        AbsolutePathBuf::from_absolute_path(&skills_root).expect("absolute skills root"),
        SkillScope::User,
        Arc::clone(&LOCAL_FS),
    ))
    .await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot
            .skills
            .into_iter()
            .map(|skill| skill.name)
            .collect::<Vec<_>>(),
        vec!["skill"]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn ancestor_symlink_roots_preserve_inherited_plugin_namespace() {
    let root = TempDir::new().expect("root temp dir");
    let plugin_root = root.path().join("a/b/c/d/e/f/outer");
    let skills_root = plugin_root.join("skills");
    write_manifest(&plugin_root, r#"{"name":"outer"}"#);
    write_skill(&skills_root, "direct");
    std::os::unix::fs::symlink(root.path(), skills_root.join("ancestor"))
        .expect("create ancestor directory symlink");

    let snapshot = load_host_skill_root(HostSkillRoot::host(
        AbsolutePathBuf::from_absolute_path(&skills_root).expect("absolute skills root"),
        SkillScope::User,
        Arc::clone(&LOCAL_FS),
    ))
    .await;

    assert_eq!(snapshot.errors, Vec::new());
    assert_eq!(
        snapshot
            .skills
            .into_iter()
            .map(|skill| skill.name)
            .collect::<Vec<_>>(),
        vec!["outer:skill"]
    );
}
