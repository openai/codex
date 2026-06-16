use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tempfile::tempdir;

use super::*;
use crate::test_support::write_file;
use crate::test_support::write_openai_curated_marketplace;

struct CatalogFixture {
    _temp: TempDir,
    root: AbsolutePathBuf,
    revision_path: AbsolutePathBuf,
    catalog: PluginCatalog,
}

impl CatalogFixture {
    fn new() -> Self {
        let temp = tempdir().expect("tempdir");
        write_openai_curated_marketplace(temp.path(), &["sample"]);
        let revision_path = temp.path().join(".revision");
        write_file(&revision_path, "one");
        Self {
            root: AbsolutePathBuf::try_from(temp.path().to_path_buf()).expect("absolute root"),
            revision_path: AbsolutePathBuf::try_from(revision_path)
                .expect("absolute revision path"),
            catalog: PluginCatalog::new(/*restriction_product*/ Some(Product::Codex)),
            _temp: temp,
        }
    }

    fn snapshot(&self) -> PluginCatalogSnapshot {
        self.catalog
            .snapshot(&[PluginCatalogSource::filesystem_root(
                self.root.clone(),
                PluginCatalogLoadMode::for_revision(PluginCatalogRevision::read(
                    self.revision_path.clone(),
                )),
            )])
            .expect("snapshot")
    }

    fn set_plugins(&self, names: &[&str]) {
        write_openai_curated_marketplace(self.root.as_path(), names);
    }

    fn set_revision(&self, revision: &str) {
        write_file(self.revision_path.as_path(), revision);
    }

    fn write_app(&self, connector_id: &str) {
        write_file(
            &self.root.join("plugins/sample/.app.json"),
            &format!(r#"{{"apps":{{"sample":{{"id":"{connector_id}"}}}}}}"#),
        );
    }
}

async fn snapshot_state(snapshot: PluginCatalogSnapshot) -> (Vec<String>, Vec<String>) {
    let plugin_names = snapshot
        .marketplace_outcome()
        .marketplaces
        .into_iter()
        .flat_map(|marketplace| marketplace.plugins)
        .map(|plugin| plugin.name)
        .collect();
    let connector_ids = snapshot
        .capability_facts("sample@openai-curated")
        .await
        .expect("summary should load")
        .expect("sample entry should exist")
        .summary
        .app_connector_ids
        .into_iter()
        .map(|connector_id| connector_id.0)
        .collect();
    (plugin_names, connector_ids)
}

async fn assert_snapshot(
    snapshot: PluginCatalogSnapshot,
    plugin_names: &[&str],
    connector_id: &str,
) {
    assert_eq!(
        snapshot_state(snapshot).await,
        (
            plugin_names.iter().map(ToString::to_string).collect(),
            vec![connector_id.to_string()],
        )
    );
}

#[tokio::test]
async fn reuses_revisioned_fragments_and_rebuilds_unrevisioned_sources() {
    let fixture = CatalogFixture::new();
    fixture.write_app("connector_first");
    assert_snapshot(fixture.snapshot(), &["sample"], "connector_first").await;

    fixture.set_plugins(&["sample", "second"]);
    fixture.write_app("connector_second");
    assert_snapshot(fixture.snapshot(), &["sample"], "connector_first").await;

    fixture.set_revision("two");
    assert_snapshot(
        fixture.snapshot(),
        &["sample", "second"],
        "connector_second",
    )
    .await;

    fixture.set_plugins(&["sample", "second", "third"]);
    fixture.write_app("connector_third");
    let rebuilt = fixture
        .catalog
        .snapshot(&[PluginCatalogSource::filesystem_root(
            fixture.root.clone(),
            PluginCatalogLoadMode::AlwaysRebuild,
        )])
        .expect("always rebuild snapshot");
    assert_snapshot(rebuilt, &["sample", "second", "third"], "connector_third").await;
    assert_snapshot(
        fixture.snapshot(),
        &["sample", "second", "third"],
        "connector_third",
    )
    .await;
}

#[tokio::test]
async fn rejects_a_stale_snapshot_before_lazy_capability_loading() {
    let fixture = CatalogFixture::new();
    let stale_snapshot = fixture.snapshot();
    fixture.set_revision("two");

    assert!(matches!(
        stale_snapshot
            .capability_facts("sample@openai-curated")
            .await,
        Err(PluginCatalogCapabilityError::SourceChanged)
    ));
}

#[tokio::test]
async fn retries_capability_load_errors() {
    let fixture = CatalogFixture::new();
    let snapshot = fixture.snapshot();
    let skill_path = fixture.root.join("plugins/sample/skills/SKILL.md");
    write_file(&skill_path, "---\nname: invalid");

    assert!(matches!(
        snapshot.capability_facts("sample@openai-curated").await,
        Err(PluginCatalogCapabilityError::InvalidPlugin(_))
    ));

    write_file(&skill_path, "---\nname: sample\ndescription: sample\n---\n");
    assert_snapshot(snapshot, &["sample"], "connector_calendar").await;
}
