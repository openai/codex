use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use super::*;
use crate::test_support::write_file;
use crate::test_support::write_openai_curated_marketplace;

fn revisioned_source(
    root: &AbsolutePathBuf,
    revision_path: &AbsolutePathBuf,
) -> PluginCatalogSource {
    PluginCatalogSource::filesystem_root(
        root.clone(),
        PluginCatalogLoadMode::for_revision(PluginCatalogRevision::read(revision_path.clone())),
    )
}

fn plugin_names(snapshot: PluginCatalogSnapshot) -> Vec<String> {
    snapshot
        .marketplace_outcome()
        .marketplaces
        .into_iter()
        .flat_map(|marketplace| marketplace.plugins)
        .map(|plugin| plugin.name)
        .collect()
}

#[test]
fn matching_revision_reuses_membership_and_new_revision_reloads_it() {
    let temp = tempdir().expect("tempdir");
    write_openai_curated_marketplace(temp.path(), &["sample"]);
    let revision_path = temp.path().join(".revision");
    write_file(&revision_path, "one");
    let root = AbsolutePathBuf::try_from(temp.path().to_path_buf()).expect("absolute root");
    let revision_path = AbsolutePathBuf::try_from(revision_path).expect("absolute revision path");
    let catalog = PluginCatalog::default();

    let first = catalog
        .snapshot(&[revisioned_source(&root, &revision_path)])
        .expect("first snapshot");
    assert_eq!(plugin_names(first), vec!["sample"]);

    write_openai_curated_marketplace(temp.path(), &["sample", "second"]);
    let matching_revision = catalog
        .snapshot(&[revisioned_source(&root, &revision_path)])
        .expect("matching revision snapshot");
    assert_eq!(plugin_names(matching_revision), vec!["sample"]);

    write_file(revision_path.as_path(), "two");
    let changed_revision = catalog
        .snapshot(&[revisioned_source(&root, &revision_path)])
        .expect("changed revision snapshot");
    assert_eq!(plugin_names(changed_revision), vec!["sample", "second"]);

    write_openai_curated_marketplace(temp.path(), &["sample", "second", "third"]);
    let rebuilt = catalog
        .snapshot(&[PluginCatalogSource::filesystem_root(
            root.clone(),
            PluginCatalogLoadMode::AlwaysRebuild,
        )])
        .expect("always rebuild snapshot");
    assert_eq!(plugin_names(rebuilt), vec!["sample", "second", "third"]);

    let revisioned_again = catalog
        .snapshot(&[revisioned_source(&root, &revision_path)])
        .expect("revisioned snapshot after rebuild");
    assert_eq!(
        plugin_names(revisioned_again),
        vec!["sample", "second", "third"]
    );
}
